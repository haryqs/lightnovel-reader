// P1 UI 冒烟：驱动真实前端，覆盖 smoke:p0（桥接层）未覆盖的 UI 交互——
// 书库卡片开书 → 翻页热区 → 鼠标划词建高亮（UI 级 + 重开持久化）→ 真实 Calibre 库读取。
// 仍无法覆盖系统文件/文件夹选择器本身（原生对话框），那条 import 路径由 smoke:p0 用路径版验证。
import { spawn } from 'node:child_process'
import { existsSync, mkdtempSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { tmpdir } from 'node:os'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const nativeDriver = join(process.env.LOCALAPPDATA, 'lightnovel-reader-tools', 'msedgedriver', '149.0.4022.62', 'msedgedriver.exe')
const application = join(repoRoot, 'target', 'debug', 'reader.exe')
const fixturesDir = join(tmpdir(), 'lightnovel-reader-smoke-epubs')
const vol1 = join(fixturesDir, 'one', 'smoke-test-lightnovel-vol1.epub')
const calibreLib = process.env.CALIBRE_LIB || 'F:\\Calibre书库'
const appDataDir = mkdtempSync(join(tmpdir(), 'lnr-p1-'))
const server = 'http://127.0.0.1:4456'

for (const f of [nativeDriver, application, vol1]) {
  if (!existsSync(f)) { console.error(`p1: missing ${f}`); process.exit(1) }
}

const driver = spawn('tauri-driver', ['--native-driver', nativeDriver, '--port', '4456', '--native-port', '9527'], {
  cwd: repoRoot, env: { ...process.env, LIGHTNOVEL_READER_APP_DATA_DIR: appDataDir }, stdio: ['ignore','pipe','pipe'], windowsHide: true,
})
let dlog = ''
driver.stdout?.on('data', c => dlog += c); driver.stderr?.on('data', c => dlog += c)
const delay = ms => new Promise(r => setTimeout(r, ms))

async function req(path, opt = {}) {
  const r = await fetch(server + path, { method: opt.method || 'GET', headers: opt.body ? {'content-type':'application/json'} : undefined, body: opt.body ? JSON.stringify(opt.body) : undefined })
  const t = await r.text(); const p = t ? JSON.parse(t) : {}
  if (!r.ok || p?.value?.error) throw new Error(`${path}: ${t}`)
  return p
}
let sid = null
const exec = async (script, args=[]) => (await req(`/session/${sid}/execute/sync`, { method:'POST', body:{ script, args } })).value
const execA = async (script, args=[]) => (await req(`/session/${sid}/execute/async`, { method:'POST', body:{ script, args } })).value
async function invoke(command, params={}) {
  const r = await execA(`const done=arguments[arguments.length-1];(async()=>{const inv=window.__TAURI__?.core?.invoke||window.__TAURI_INTERNALS__?.invoke;done({ok:true,value:await inv(arguments[0],arguments[1]||{})})})().catch(e=>done({ok:false,message:e?.message||String(e)}))`, [command, params])
  if (!r?.ok) throw new Error(`${command}: ${r?.message}`)
  return r.value
}
async function waitFor(label, producer, predicate, timeoutMs=12000) {
  const t0 = Date.now(); let last
  while (Date.now()-t0 < timeoutMs) { last = await producer(); if (predicate(last)) return last; await delay(250) }
  throw new Error(`${label} timed out: ${JSON.stringify(last)}`)
}
function assert(c, m, d) { if (!c) throw new Error(d ? `${m}: ${JSON.stringify(d)}` : m) }

async function main() {
  for (let i=0;i<80;i++){ try { await req('/status'); break } catch { await delay(250) } }
  const s = await req('/session', { method:'POST', timeoutMs:30000, body:{ capabilities:{ alwaysMatch:{ browserName:'wry', 'tauri:options':{ application } } } } })
  sid = s?.value?.sessionId || s?.sessionId
  await waitFor('invoke ready', () => exec(`return { r: document.readyState, t: !!(window.__TAURI__?.core?.invoke) }`), v => v.t && v.r !== 'loading', 15000)

  // 准备一本书入库
  const imp = await invoke('library_import', { path: vol1 })
  const bookId = imp.book.id

  // —— 1) 书库卡片开书（真实前端流程，而非直接调命令）——
  await exec(`document.querySelector('#btn-library')?.click(); return true`)
  await waitFor('book card', () => exec(`return document.querySelectorAll('#library-grid .book-card').length`), n => n >= 1)
  await exec(`document.querySelector('#library-grid .book-card')?.click(); return true`)
  const opened = await waitFor('chapter rendered',
    () => exec(`const c=document.querySelector('.reader-content'); return { has:!!c, len:(c?.textContent||'').length, libHidden: document.querySelector('#library-view')?.hidden===true }`),
    v => v.has && v.len > 20 && v.libHidden)
  assert(opened.len > 20, 'reader content empty after opening from shelf', opened)

  // —— 2) 翻页热区 ——
  const pageBefore = await exec(`return document.querySelector('#progress-label')?.textContent || ''`)
  const zones = await exec(`return { next: !!document.querySelector('#next-zone'), prev: !!document.querySelector('#prev-zone'), nextHidden: document.querySelector('#next-zone')?.hidden, }`)
  assert(zones.next && zones.prev && zones.nextHidden === false, 'page-turn hotzones missing/hidden', zones)
  await exec(`document.querySelector('#next-zone')?.click(); return true`)
  await delay(500)
  await exec(`document.querySelector('#prev-zone')?.click(); return true`)
  await delay(300)
  const noBlank = await exec(`const c=document.querySelector('.reader-content'); return (c?.textContent||'').length > 20`)
  assert(noBlank, 'page turned to blank content')

  // —— 3) 鼠标划词建高亮（UI 级）——
  const hl = await execA(`
    const done = arguments[arguments.length-1]
    ;(async()=>{
      const content = document.querySelector('.reader-content')
      if (!content) return done({ ok:false, message:'no .reader-content' })
      const walker = document.createTreeWalker(content, NodeFilter.SHOW_TEXT)
      let node = null
      while (walker.nextNode()) { if ((walker.currentNode.textContent||'').trim().length >= 6) { node = walker.currentNode; break } }
      if (!node) return done({ ok:false, message:'no text node' })
      const range = document.createRange(); range.setStart(node, 0); range.setEnd(node, 5)
      const sel = window.getSelection(); sel.removeAllRanges(); sel.addRange(range)
      const r = range.getBoundingClientRect()
      content.dispatchEvent(new MouseEvent('mouseup', { bubbles:true, clientX: r.left+2, clientY: r.top+2 }))
      await new Promise(res=>setTimeout(res, 120))
      const swatch = document.querySelector('.color-popup button')
      if (!swatch) return done({ ok:false, message:'color popup did not appear' })
      swatch.dispatchEvent(new MouseEvent('mousedown', { bubbles:true }))
      await new Promise(res=>setTimeout(res, 300))
      done({ ok:true, marks: document.querySelectorAll('mark[data-annotation-id]').length })
    })().catch(e=>done({ ok:false, message:e?.message||String(e) }))
  `)
  assert(hl.ok && hl.marks >= 1, 'selection→highlight did not create a mark', hl)
  const anns = await invoke('list_annotations', { bookId })
  assert(anns.length >= 1, 'highlight not persisted to DB', anns)

  // 重开同书，标注 mark 应重新渲染
  await exec(`document.querySelector('#btn-library')?.click(); return true`)
  await waitFor('shelf reopened', () => exec(`return document.querySelector('#library-view')?.hidden===false`), v=>v===true)
  await exec(`document.querySelector('#library-grid .book-card')?.click(); return true`)
  const restored = await waitFor('marks restored on reopen',
    () => exec(`return document.querySelectorAll('mark[data-annotation-id]').length`),
    n => n >= 1, 12000)
  assert(restored >= 1, 'highlight mark not restored after reopen')

  // —— 4) 真实 Calibre 库读取 ——
  const calibre = await invoke('list_calibre_books', { library: calibreLib })
  assert(Array.isArray(calibre) && calibre.length >= 1, 'Calibre library returned no EPUBs', { count: calibre?.length })

  console.log(JSON.stringify({ ok:true, opened, marks: hl.marks, annotations: anns.length, restoredMarks: restored, calibreBooks: calibre.length, sampleCalibre: calibre.slice(0,3).map(b=>b.title) }, null, 2))
  console.log('tauri-p1-smoke: OK')
}

try { await main() } catch (e) {
  console.error('tauri-p1-smoke: FAILED'); console.error(e?.message||e); console.error('driver:', dlog.slice(-600)); process.exitCode = 1
} finally {
  try { if (sid) await req(`/session/${sid}`, { method:'DELETE' }) } catch {}
  if (driver.pid) spawn('taskkill', ['/PID', String(driver.pid), '/T', '/F'], { stdio:'ignore', windowsHide:true })
}
