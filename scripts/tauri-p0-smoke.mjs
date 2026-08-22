import { spawn } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from 'node:fs'
import { dirname, join, resolve, sep } from 'node:path'
import process from 'node:process'
import { tmpdir } from 'node:os'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const args = process.argv.slice(2)

function readOption(name, fallback) {
  const eq = args.find((arg) => arg.startsWith(`${name}=`))
  if (eq) return eq.slice(name.length + 1)
  const idx = args.indexOf(name)
  if (idx >= 0 && idx + 1 < args.length) return args[idx + 1]
  return fallback
}

function hasFlag(name) {
  return args.includes(name)
}

function defaultNativeDriver() {
  const localAppData = process.env.LOCALAPPDATA
  if (!localAppData) return ''
  return join(
    localAppData,
    'lightnovel-reader-tools',
    'msedgedriver',
    '149.0.4022.62',
    'msedgedriver.exe',
  )
}

const driverBinary = readOption('--tauri-driver', process.env.TAURI_DRIVER || 'tauri-driver')
const nativeDriver = resolve(readOption('--native-driver', process.env.MSEDGEDRIVER || defaultNativeDriver()))
const application = resolve(
  readOption(
    '--application',
    process.env.TAURI_APP_PATH || join(repoRoot, 'target', 'debug', process.platform === 'win32' ? 'reader.exe' : 'reader'),
  ),
)
const driverPort = Number(readOption('--driver-port', process.env.TAURI_DRIVER_PORT || '4444'))
const nativePort = Number(readOption('--native-port', process.env.TAURI_NATIVE_DRIVER_PORT || '9515'))
const fixturesDir = resolve(
  readOption('--fixtures-dir', process.env.SMOKE_FIXTURES_DIR || join(tmpdir(), 'lightnovel-reader-smoke-epubs')),
)
const customAppDataDir = readOption('--app-data-dir', process.env.LIGHTNOVEL_READER_APP_DATA_DIR || '')
const appDataDir = customAppDataDir
  ? resolve(customAppDataDir)
  : mkdtempSync(join(tmpdir(), 'lightnovel-reader-p0-smoke-'))
const autoAppDataDir = !customAppDataDir
const keepOpen = hasFlag('--keep-open')
const keepData = hasFlag('--keep-data') || !autoAppDataDir
const skipFixtures = hasFlag('--skip-fixtures')
const server = `http://127.0.0.1:${driverPort}`

const vol1 = join(fixturesDir, 'one', 'smoke-test-lightnovel-vol1.epub')
const folderVol1Copy = join(fixturesDir, 'folder', 'Smoke Test Series', 'Vol01', 'smoke-test-lightnovel-vol1-copy.epub')
const folderVol2 = join(fixturesDir, 'folder', 'Smoke Test Series', 'Vol02', 'smoke-test-lightnovel-vol2.epub')

function failPreflight(message) {
  console.error(`tauri-p0-smoke: ${message}`)
  process.exit(1)
}

if (!Number.isInteger(driverPort) || driverPort <= 0) {
  failPreflight(`invalid --driver-port: ${driverPort}`)
}
if (!Number.isInteger(nativePort) || nativePort <= 0) {
  failPreflight(`invalid --native-port: ${nativePort}`)
}
if (!existsSync(nativeDriver)) {
  failPreflight(`native WebDriver not found: ${nativeDriver}`)
}
if (!existsSync(application)) {
  failPreflight(
    `Tauri debug app not found: ${application}\n` +
      'Build it first with: npm.cmd run tauri -- build --debug --no-bundle',
  )
}

function runPowerShell(script, scriptArgs) {
  return new Promise((resolveRun, rejectRun) => {
    const child = spawn(
      'powershell',
      ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', script, ...scriptArgs],
      {
        cwd: repoRoot,
        stdio: ['ignore', 'pipe', 'pipe'],
        windowsHide: true,
      },
    )
    let output = ''
    child.stdout?.on('data', (chunk) => {
      output += String(chunk)
    })
    child.stderr?.on('data', (chunk) => {
      output += String(chunk)
    })
    child.once('error', rejectRun)
    child.once('exit', (code, signal) => {
      if (code === 0) {
        resolveRun(output)
      } else {
        rejectRun(new Error(`PowerShell exited with ${code ?? signal}:\n${output.trim()}`))
      }
    })
  })
}

if (!skipFixtures) {
  await runPowerShell(join(repoRoot, 'scripts', 'new-smoke-epubs.ps1'), ['-OutDir', fixturesDir])
}
for (const fixture of [vol1, folderVol1Copy, folderVol2]) {
  if (!existsSync(fixture)) {
    failPreflight(`fixture EPUB not found: ${fixture}\nRun: npm.cmd run smoke:fixtures`)
  }
}
mkdirSync(appDataDir, { recursive: true })

const stdout = []
const stderr = []
let driverExited = false
let driverExitCode = null
let driverExitSignal = null

const driver = spawn(
  driverBinary,
  [
    '--native-driver',
    nativeDriver,
    '--port',
    String(driverPort),
    '--native-port',
    String(nativePort),
  ],
  {
    cwd: repoRoot,
    env: {
      ...process.env,
      LIGHTNOVEL_READER_APP_DATA_DIR: appDataDir,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
    windowsHide: true,
  },
)

driver.stdout?.on('data', (chunk) => collect(stdout, chunk))
driver.stderr?.on('data', (chunk) => collect(stderr, chunk))
driver.once('exit', (code, signal) => {
  driverExited = true
  driverExitCode = code
  driverExitSignal = signal
})

function collect(target, chunk) {
  target.push(String(chunk))
  if (target.length > 100) target.splice(0, target.length - 100)
}

function tail(lines) {
  return lines.join('').trim()
}

function delay(ms) {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, ms))
}

async function request(path, { method = 'GET', body, timeoutMs = 10_000 } = {}) {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), timeoutMs)
  try {
    const response = await fetch(`${server}${path}`, {
      method,
      headers: body ? { 'content-type': 'application/json' } : undefined,
      body: body ? JSON.stringify(body) : undefined,
      signal: controller.signal,
    })
    const text = await response.text()
    let payload = {}
    if (text) {
      try {
        payload = JSON.parse(text)
      } catch {
        payload = { raw: text }
      }
    }
    if (!response.ok) {
      throw new Error(`${method} ${path} failed with ${response.status}: ${text}`)
    }
    if (payload?.value?.error) {
      throw new Error(`${method} ${path} failed: ${payload.value.message || payload.value.error}`)
    }
    return payload
  } finally {
    clearTimeout(timeout)
  }
}

async function waitForDriver() {
  for (let i = 0; i < 80; i += 1) {
    if (driverExited) {
      throw new Error(
        `tauri-driver exited early (${driverExitCode ?? driverExitSignal}).\n${tail(stderr) || tail(stdout)}`,
      )
    }
    try {
      await request('/status', { timeoutMs: 1_000 })
      return
    } catch {
      await delay(250)
    }
  }
  throw new Error(`tauri-driver did not become ready on ${server}\n${tail(stderr) || tail(stdout)}`)
}

function sessionIdFrom(payload) {
  return payload?.value?.sessionId || payload?.sessionId || null
}

let sessionId = null

async function createSession() {
  const session = await request('/session', {
    method: 'POST',
    timeoutMs: 30_000,
    body: {
      capabilities: {
        alwaysMatch: {
          browserName: 'wry',
          'tauri:options': { application },
        },
      },
    },
  })
  sessionId = sessionIdFrom(session)
  assertCheck(sessionId, 'WebDriver did not return a session id', session)
}

async function deleteSession() {
  if (!sessionId) return
  const deleting = sessionId
  sessionId = null
  try {
    await request(`/session/${deleting}`, { method: 'DELETE', timeoutMs: 5_000 })
  } catch {
    // Best effort; the driver process is still terminated during final cleanup.
  }
}

async function execute(script, scriptArgs = []) {
  const payload = await request(`/session/${sessionId}/execute/sync`, {
    method: 'POST',
    body: { script, args: scriptArgs },
  })
  return payload.value
}

async function executeAsync(script, scriptArgs = [], timeoutMs = 20_000) {
  const payload = await request(`/session/${sessionId}/execute/async`, {
    method: 'POST',
    timeoutMs,
    body: { script, args: scriptArgs },
  })
  return payload.value
}

async function invoke(command, params = {}, timeoutMs = 20_000) {
  const result = await executeAsync(
    `
      const command = arguments[0]
      const params = arguments[1] || {}
      const done = arguments[arguments.length - 1]
      ;(async () => {
        const invoke = window.__TAURI__?.core?.invoke || window.__TAURI_INTERNALS__?.invoke
        if (!invoke) {
          throw new Error('Tauri invoke is not available')
        }
        const value = await invoke(command, params)
        done({ ok: true, value })
      })().catch((error) => {
        done({
          ok: false,
          message: error?.message || String(error),
          stack: error?.stack || '',
        })
      })
    `,
    [command, params],
    timeoutMs,
  )
  if (!result?.ok) {
    throw new Error(`${command} failed: ${result?.message || JSON.stringify(result)}\n${result?.stack || ''}`)
  }
  return result.value
}

async function waitForValue(label, producer, predicate, timeoutMs = 10_000) {
  const started = Date.now()
  let lastValue
  while (Date.now() - started < timeoutMs) {
    lastValue = await producer()
    if (predicate(lastValue)) return lastValue
    await delay(250)
  }
  throw new Error(`${label} timed out. Last value: ${JSON.stringify(lastValue)}`)
}

async function waitForInvokeReady(label) {
  return await waitForValue(
    label,
    () =>
      execute(`
        const tauriKeys = Object.keys(window.__TAURI__ || {})
        const coreKeys = Object.keys(window.__TAURI__?.core || {})
        return {
          title: document.title,
          readyState: document.readyState,
          hasInvoke: !!(window.__TAURI__?.core?.invoke || window.__TAURI_INTERNALS__?.invoke),
          tauriKeys,
          coreKeys,
        }
      `),
    (value) => value.hasInvoke && value.readyState !== 'loading',
    15_000,
  )
}

function assertCheck(condition, message, details) {
  if (!condition) {
    throw new Error(details ? `${message}: ${JSON.stringify(details)}` : message)
  }
}

function assertBookMeta(book, expected) {
  assertCheck(book.title === expected.title, 'unexpected book title', book)
  assertCheck(book.author === 'Smoke Tester', 'unexpected book author', book)
  assertCheck(book.language === 'zh-CN', 'unexpected book language', book)
  assertCheck(book.series === 'Smoke Test Series', 'unexpected book series', book)
  assertCheck(book.seriesIndex === expected.seriesIndex, 'unexpected book series index', book)
  assertCheck(!!book.description, 'book description was not imported', book)
  assertCheck(!!book.filePath && existsSync(book.filePath), 'book file path is missing on disk', book)
  assertCheck(!!book.coverPath && existsSync(book.coverPath), 'book cover path is missing on disk', book)
  // 样本封面是有效 PNG → 应生成缩略图（v0.4 封面缩略图）
  assertCheck(!!book.thumbPath && existsSync(book.thumbPath), 'book thumbnail was not generated', book)
  assertCheck(book.thumbPath.endsWith('_thumb.png'), 'unexpected thumbnail path', book)
}

function findBook(books, title) {
  return books.find((book) => book.title === title)
}

function parseCacheState(bookId) {
  const dir = join(appDataDir, 'cache', 'parsed', 'v1', bookId)
  const infoPath = join(dir, 'info.json')
  const chapterDir = join(dir, 'ch')
  const chapterFiles = existsSync(chapterDir)
    ? readdirSync(chapterDir)
        .filter((name) => name.endsWith('.html'))
        .map((name) => join(chapterDir, name))
    : []
  return {
    dir,
    infoPath,
    chapterDir,
    hasDir: existsSync(dir),
    hasInfo: existsSync(infoPath),
    chapterFiles,
    chapterCount: chapterFiles.length,
  }
}

function assertParseCache(bookId) {
  const state = parseCacheState(bookId)
  assertCheck(state.hasDir, 'parse cache directory was not created', state)
  assertCheck(state.hasInfo, 'parse cache info.json was not created', state)
  assertCheck(state.chapterCount > 0, 'parse cache chapter HTML was not created', state)
  const info = JSON.parse(readFileSync(state.infoPath, 'utf8'))
  assertCheck(info?.metadata?.title === 'Smoke Test Light Novel Vol.1', 'parse cache info.json title mismatch', {
    state,
    info,
  })
  const chapterSizes = state.chapterFiles.map((file) => ({ file, size: statSync(file).size }))
  assertCheck(chapterSizes.every((item) => item.size > 0), 'parse cache chapter file is empty', chapterSizes)
  return { ...state, infoTitle: info.metadata.title, chapterSizes }
}

async function verifyChapterImageLoads(chapterHtml) {
  const result = await executeAsync(
    `
      const html = arguments[0]
      const done = arguments[arguments.length - 1]
      ;(async () => {
        const box = document.createElement('div')
        box.id = 'p0-inline-image-smoke'
        box.style.cssText = 'position:absolute;left:-10000px;top:0;width:360px;height:auto;overflow:hidden'
        box.innerHTML = html
        document.body.appendChild(box)
        const imgs = Array.from(box.querySelectorAll('img'))
        if (imgs.length === 0) {
          box.remove()
          done({ ok: false, message: 'chapter HTML did not render any img tags' })
          return
        }
        await Promise.all(imgs.map((img) => {
          if (img.complete) return Promise.resolve()
          return new Promise((resolveImage) => {
            const finish = () => resolveImage()
            img.addEventListener('load', finish, { once: true })
            img.addEventListener('error', finish, { once: true })
            setTimeout(finish, 5000)
          })
        }))
        const images = imgs.map((img) => ({
          src: img.currentSrc || img.src,
          complete: img.complete,
          naturalWidth: img.naturalWidth,
          naturalHeight: img.naturalHeight,
        }))
        box.remove()
        done({
          ok: images.every((img) => img.complete && img.naturalWidth > 0 && img.naturalHeight > 0),
          images,
        })
      })().catch((error) => {
        done({ ok: false, message: error?.message || String(error), stack: error?.stack || '' })
      })
    `,
    [chapterHtml],
    10_000,
  )
  assertCheck(result?.ok, 'inline chapter image failed to load through reader-img', result)
  return result.images
}

async function cleanup() {
  await deleteSession()
  if (!driverExited && driver.pid) {
    if (process.platform === 'win32') {
      await new Promise((resolveKill) => {
        const killer = spawn('taskkill', ['/PID', String(driver.pid), '/T', '/F'], {
          stdio: 'ignore',
          windowsHide: true,
        })
        killer.once('exit', resolveKill)
        killer.once('error', resolveKill)
      })
    } else {
      driver.kill('SIGTERM')
    }
  }

  if (!keepData && autoAppDataDir) {
    const tmpRoot = resolve(tmpdir())
    const resolved = resolve(appDataDir)
    if (resolved.startsWith(`${tmpRoot}${sep}`)) {
      rmSync(resolved, { recursive: true, force: true })
    }
  }
}

async function runFirstSession() {
  await createSession()
  const boot = await waitForInvokeReady('initial Tauri invoke')

  const importedVol1 = await invoke('library_import', { path: vol1 }, 30_000)
  assertCheck(importedVol1.duplicate === false, 'first Vol.1 import should not be duplicate', importedVol1)
  assertBookMeta(importedVol1.book, {
    title: 'Smoke Test Light Novel Vol.1',
    seriesIndex: 1,
  })

  const duplicateVol1 = await invoke('library_import', { path: vol1 }, 30_000)
  assertCheck(duplicateVol1.duplicate === true, 'repeat Vol.1 import should be duplicate', duplicateVol1)
  assertCheck(duplicateVol1.book.id === importedVol1.book.id, 'duplicate import returned a different id', duplicateVol1)

  const duplicateFolderCopy = await invoke('library_import', { path: folderVol1Copy }, 30_000)
  assertCheck(
    duplicateFolderCopy.duplicate === true,
    'folder Vol.1 copy should be detected as duplicate',
    duplicateFolderCopy,
  )

  const importedVol2 = await invoke('library_import', { path: folderVol2 }, 30_000)
  assertCheck(importedVol2.duplicate === false, 'Vol.2 folder import should not be duplicate', importedVol2)
  assertBookMeta(importedVol2.book, {
    title: 'Smoke Test Light Novel Vol.2',
    seriesIndex: 2,
  })

  const books = await invoke('library_list', {}, 20_000)
  assertCheck(Array.isArray(books) && books.length === 2, 'library should contain exactly two books', books)
  const vol1Book = findBook(books, 'Smoke Test Light Novel Vol.1')
  const vol2Book = findBook(books, 'Smoke Test Light Novel Vol.2')
  assertCheck(!!vol1Book && !!vol2Book, 'library list is missing smoke books', books)
  assertBookMeta(vol1Book, { title: 'Smoke Test Light Novel Vol.1', seriesIndex: 1 })
  assertBookMeta(vol2Book, { title: 'Smoke Test Light Novel Vol.2', seriesIndex: 2 })

  const firstOpenStarted = performance.now()
  const opened = await invoke('library_open', { id: vol1Book.id }, 30_000)
  const firstOpenMs = performance.now() - firstOpenStarted
  assertCheck(opened.bookId === vol1Book.id, 'library_open returned unexpected bookId', opened)
  assertCheck(opened.info?.metadata?.title === vol1Book.title, 'opened book metadata mismatch', opened)
  const firstHref = opened.info?.spine?.[0]?.href
  assertCheck(!!firstHref, 'opened book has no first spine href', opened)

  const chapterHtml = await invoke('get_chapter', { href: firstHref }, 30_000)
  assertCheck(
    typeof chapterHtml === 'string' && chapterHtml.includes('Volume one is used'),
    'chapter HTML does not contain expected smoke text',
    { firstHref, chapterHtml: chapterHtml?.slice?.(0, 240) },
  )
  assertCheck(chapterHtml.includes('reader-img'), 'chapter HTML did not rewrite inline image to reader-img', {
    firstHref,
    chapterHtml: chapterHtml?.slice?.(0, 500),
  })
  const inlineImages = await verifyChapterImageLoads(chapterHtml)
  const parseCache = assertParseCache(vol1Book.id)

  const now = Date.now()
  const progress = {
    bookId: vol1Book.id,
    chapterHref: firstHref,
    chapterProgress: 0.42,
    percentage: 0.42,
    updatedAt: now,
  }
  await invoke('save_progress', { progress }, 20_000)
  const savedProgress = await invoke('get_progress', { bookId: vol1Book.id }, 20_000)
  assertCheck(savedProgress?.bookId === progress.bookId, 'saved progress bookId mismatch', savedProgress)
  assertCheck(savedProgress?.chapterHref === progress.chapterHref, 'saved progress chapter href mismatch', savedProgress)
  assertCheck(Math.abs(savedProgress?.percentage - 0.42) < 0.0001, 'saved progress percentage mismatch', savedProgress)

  const annotation = {
    id: `smoke-ann-${now}`,
    bookId: vol1Book.id,
    kind: 'highlight',
    color: 'yellow',
    locator: {
      chapterHref: firstHref,
      anchor: {
        start: 1,
        end: 12,
        exact: 'Smoke Test',
        prefix: '',
        suffix: 'Light Novel',
      },
    },
    note: 'P0 smoke annotation',
    createdAt: now,
    updatedAt: now,
  }
  await invoke('save_annotation', { annotation }, 20_000)
  const annotations = await invoke('list_annotations', { bookId: vol1Book.id }, 20_000)
  assertCheck(
    annotations.some((item) => item.id === annotation.id && item.locator?.chapterHref === firstHref),
    'saved annotation was not listed',
    annotations,
  )

  await invoke('library_touch_last_read', { id: vol1Book.id }, 20_000)

  await execute(`document.querySelector('#btn-library')?.click(); return true`)
  const libraryOrganizeInitial = await waitForValue(
    'organized shelf with imported books',
    () => execute(`
      return {
        titles: Array.from(document.querySelectorAll('#library-grid .book-card .title')).map((node) => node.textContent || ''),
        filterValue: document.querySelector('#library-filter')?.value || '',
        sortValue: document.querySelector('#library-sort')?.value || '',
        summary: document.querySelector('#library-result-summary')?.textContent || '',
      }
    `),
    (value) => value.titles.length === 2,
    20_000,
  )
  assertCheck(
    libraryOrganizeInitial.titles[0] === vol1Book.title,
    'recently read book should sort first',
    libraryOrganizeInitial,
  )
  assertCheck(libraryOrganizeInitial.summary === '2 本', 'unexpected initial shelf summary', libraryOrganizeInitial)

  const libraryOrganizeUnread = await execute(`
    const filter = document.querySelector('#library-filter')
    filter.value = 'unread'
    filter.dispatchEvent(new Event('change', { bubbles: true }))
    return {
      titles: Array.from(document.querySelectorAll('#library-grid .book-card .title')).map((node) => node.textContent || ''),
      summary: document.querySelector('#library-result-summary')?.textContent || '',
    }
  `)
  assertCheck(
    libraryOrganizeUnread.titles.length === 1 && libraryOrganizeUnread.titles[0] === vol2Book.title,
    'unread filter should only keep Vol.2',
    libraryOrganizeUnread,
  )
  assertCheck(libraryOrganizeUnread.summary === '显示 1 / 2 本', 'unexpected unread summary', libraryOrganizeUnread)

  const libraryOrganizeRemote = await execute(`
    const filter = document.querySelector('#library-filter')
    filter.value = 'remote'
    filter.dispatchEvent(new Event('change', { bubbles: true }))
    return {
      cards: document.querySelectorAll('#library-grid .book-card').length,
      summary: document.querySelector('#library-result-summary')?.textContent || '',
      state: document.querySelector('#library-grid .library-state')?.textContent || '',
    }
  `)
  assertCheck(libraryOrganizeRemote.cards === 0, 'remote filter should hide local-only fixtures', libraryOrganizeRemote)
  assertCheck(libraryOrganizeRemote.summary === '显示 0 / 2 本', 'unexpected remote summary', libraryOrganizeRemote)
  assertCheck(libraryOrganizeRemote.state.includes('当前筛选下没有书'), 'filtered empty state is missing', libraryOrganizeRemote)

  const libraryOrganizeTitle = await execute(`
    const filter = document.querySelector('#library-filter')
    const sort = document.querySelector('#library-sort')
    filter.value = 'readable'
    filter.dispatchEvent(new Event('change', { bubbles: true }))
    sort.value = 'title'
    sort.dispatchEvent(new Event('change', { bubbles: true }))
    const result = {
      titles: Array.from(document.querySelectorAll('#library-grid .book-card .title')).map((node) => node.textContent || ''),
      summary: document.querySelector('#library-result-summary')?.textContent || '',
    }
    filter.value = 'all'
    filter.dispatchEvent(new Event('change', { bubbles: true }))
    sort.value = 'recent'
    sort.dispatchEvent(new Event('change', { bubbles: true }))
    return result
  `)
  assertCheck(
    libraryOrganizeTitle.titles.join('|') === `${vol1Book.title}|${vol2Book.title}`,
    'title sorting order is wrong',
    libraryOrganizeTitle,
  )

  return {
    boot,
    vol1Id: vol1Book.id,
    vol2Id: vol2Book.id,
    firstHref,
    annotationId: annotation.id,
    progress,
    firstOpenMs,
    parseCache,
    inlineImages,
    books,
    libraryOrganize: {
      initial: libraryOrganizeInitial,
      unread: libraryOrganizeUnread,
      remote: libraryOrganizeRemote,
      title: libraryOrganizeTitle,
    },
  }
}

async function runSecondSession(expected) {
  await createSession()
  const boot = await waitForInvokeReady('relaunch Tauri invoke')

  const books = await invoke('library_list', {}, 20_000)
  assertCheck(Array.isArray(books) && books.length === 2, 'relaunch library should contain two books', books)
  const vol1Book = findBook(books, 'Smoke Test Light Novel Vol.1')
  assertCheck(!!vol1Book && vol1Book.id === expected.vol1Id, 'relaunch Vol.1 id mismatch', books)

  const progress = await invoke('get_progress', { bookId: expected.vol1Id }, 20_000)
  assertCheck(progress?.bookId === expected.vol1Id, 'relaunch progress bookId mismatch', progress)
  assertCheck(progress?.chapterHref === expected.firstHref, 'relaunch progress chapter mismatch', progress)
  assertCheck(Math.abs(progress?.percentage - 0.42) < 0.0001, 'relaunch progress percentage mismatch', progress)

  const annotations = await invoke('list_annotations', { bookId: expected.vol1Id }, 20_000)
  assertCheck(
    annotations.some((item) => item.id === expected.annotationId && item.note === 'P0 smoke annotation'),
    'relaunch annotation was not restored',
    annotations,
  )

  const secondOpenStarted = performance.now()
  const opened = await invoke('library_open', { id: expected.vol1Id }, 30_000)
  const secondOpenMs = performance.now() - secondOpenStarted
  const chapterHtml = await invoke('get_chapter', { href: opened.info.spine[0].href }, 30_000)
  assertCheck(
    typeof chapterHtml === 'string' && chapterHtml.includes('progress, cover, and duplicate-import smoke testing'),
    'relaunch chapter read failed',
    { href: opened.info.spine[0].href, chapterHtml: chapterHtml?.slice?.(0, 240) },
  )
  assertCheck(chapterHtml.includes('reader-img'), 'relaunch chapter HTML lost reader-img image rewrite', {
    href: opened.info.spine[0].href,
    chapterHtml: chapterHtml?.slice?.(0, 500),
  })
  const inlineImages = await verifyChapterImageLoads(chapterHtml)
  const parseCache = assertParseCache(expected.vol1Id)

  return { boot, books, progress, annotations, secondOpenMs, inlineImages, parseCache }
}

async function main() {
  await waitForDriver()

  const first = await runFirstSession()
  await deleteSession()
  await delay(1_000)
  const second = await runSecondSession(first)

  console.log(
    JSON.stringify(
      {
        ok: true,
        application,
        nativeDriver,
        fixturesDir,
        appDataDir,
        keepOpen,
        keepData,
        checks: {
          importedBooks: first.books.map((book) => ({
            id: book.id,
            title: book.title,
            language: book.language,
            series: book.series,
            seriesIndex: book.seriesIndex,
            hasCover: !!book.coverPath,
          })),
          progress: second.progress,
          annotations: second.annotations.map((annotation) => ({
            id: annotation.id,
            kind: annotation.kind,
            note: annotation.note,
          })),
          parseCache: {
            dir: second.parseCache.dir,
            infoJson: second.parseCache.infoPath,
            chapterCount: second.parseCache.chapterCount,
            chapterSizes: second.parseCache.chapterSizes,
          },
          inlineImages: second.inlineImages,
          libraryOrganize: first.libraryOrganize,
          openTimingMs: {
            first: Number(first.firstOpenMs.toFixed(2)),
            second: Number(second.secondOpenMs.toFixed(2)),
          },
        },
      },
      null,
      2,
    ),
  )
  console.log('tauri-p0-smoke: OK')
}

try {
  await main()
} catch (error) {
  console.error('tauri-p0-smoke: FAILED')
  console.error(error?.stack || error?.message || String(error))
  const err = tail(stderr)
  const out = tail(stdout)
  if (err) console.error(`tauri-driver stderr:\n${err}`)
  if (out) console.error(`tauri-driver stdout:\n${out}`)
  process.exitCode = 1
} finally {
  if (!keepOpen) {
    await cleanup()
  } else if (!keepData) {
    console.error(`tauri-p0-smoke: keeping app open, data dir left at ${appDataDir}`)
  }
}
