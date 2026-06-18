import { spawn } from 'node:child_process'
import { existsSync, mkdtempSync, rmSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { tmpdir } from 'node:os'
import process from 'node:process'
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
const driverPort = Number(readOption('--driver-port', process.env.TAURI_DRIVER_PORT || '4462'))
const nativePort = Number(readOption('--native-port', process.env.TAURI_NATIVE_DRIVER_PORT || '9533'))
const opdsUrl = readOption(
  '--opds-url',
  process.env.OPDS_SMOKE_URL || 'https://www.gutenberg.org/ebooks/search.opds/?query=austen',
)
const expectedTitle = readOption('--title', process.env.OPDS_SMOKE_TITLE || 'Pride and Prejudice')
const appDataDir = resolve(
  readOption('--app-data-dir', process.env.LIGHTNOVEL_READER_APP_DATA_DIR || mkdtempSync(join(tmpdir(), 'lnr-opds-smoke-'))),
)
const autoAppDataDir = !process.env.LIGHTNOVEL_READER_APP_DATA_DIR && !args.some((arg) => arg === '--app-data-dir' || arg.startsWith('--app-data-dir='))
const keepOpen = hasFlag('--keep-open')
const keepData = hasFlag('--keep-data') || !autoAppDataDir
const server = `http://127.0.0.1:${driverPort}`

function failPreflight(message) {
  console.error(`tauri-opds-smoke: ${message}`)
  process.exit(1)
}

for (const [label, value] of [['driver port', driverPort], ['native port', nativePort]]) {
  if (!Number.isInteger(value) || value <= 0) failPreflight(`invalid ${label}: ${value}`)
}
if (!existsSync(nativeDriver)) failPreflight(`native WebDriver not found: ${nativeDriver}`)
if (!existsSync(application)) {
  failPreflight(
    `Tauri debug app not found: ${application}\n` +
      'Build it first with: npm.cmd run tauri -- build --debug --no-bundle',
  )
}

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
    env: { ...process.env, LIGHTNOVEL_READER_APP_DATA_DIR: appDataDir },
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
    const payload = text ? JSON.parse(text) : {}
    if (!response.ok || payload?.value?.error) {
      throw new Error(`${method} ${path} failed with ${response.status}: ${text}`)
    }
    return payload
  } finally {
    clearTimeout(timeout)
  }
}

let sessionId = null

async function execute(script, scriptArgs = [], timeoutMs = 10_000) {
  const payload = await request(`/session/${sessionId}/execute/sync`, {
    method: 'POST',
    timeoutMs,
    body: { script, args: scriptArgs },
  })
  return payload.value
}

async function executeAsync(script, scriptArgs = [], timeoutMs = 10_000) {
  const payload = await request(`/session/${sessionId}/execute/async`, {
    method: 'POST',
    timeoutMs,
    body: { script, args: scriptArgs },
  })
  return payload.value
}

async function invoke(command, params = {}) {
  const result = await executeAsync(
    `const done=arguments[arguments.length-1];
     (async()=>{
       const inv=window.__TAURI__?.core?.invoke||window.__TAURI_INTERNALS__?.invoke;
       done({ok:true,value:await inv(arguments[0],arguments[1]||{})});
     })().catch(e=>done({ok:false,message:e?.message||String(e)}))`,
    [command, params],
    120_000,
  )
  if (!result?.ok) throw new Error(`${command}: ${result?.message}`)
  return result.value
}

async function waitFor(label, producer, predicate, timeoutMs = 15_000) {
  const started = Date.now()
  let lastValue
  while (Date.now() - started < timeoutMs) {
    lastValue = await producer()
    if (predicate(lastValue)) return lastValue
    await delay(300)
  }
  throw new Error(`${label} timed out. Last value: ${JSON.stringify(lastValue)}`)
}

function assertCheck(condition, message, details) {
  if (!condition) throw new Error(details ? `${message}: ${JSON.stringify(details)}` : message)
}

async function waitForDriver() {
  for (let i = 0; i < 80; i += 1) {
    if (driverExited) {
      throw new Error(`tauri-driver exited early (${driverExitCode ?? driverExitSignal}).\n${tail(stderr) || tail(stdout)}`)
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

async function cleanup() {
  if (!keepOpen && sessionId) {
    try {
      await request(`/session/${sessionId}`, { method: 'DELETE', timeoutMs: 5_000 })
    } catch {
      // Best effort cleanup.
    }
  }
  if (!keepOpen && !driverExited && driver.pid) {
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
  if (!keepData && !keepOpen) {
    rmSync(appDataDir, { recursive: true, force: true })
  }
}

async function main() {
  await waitForDriver()
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
  sessionId = session?.value?.sessionId || session?.sessionId
  assertCheck(sessionId, 'WebDriver session did not return sessionId', session)

  await waitFor(
    'library OPDS controls ready',
    () => execute(`return {
      readyState: document.readyState,
      hasInvoke: !!(window.__TAURI__?.core?.invoke || window.__TAURI_INTERNALS__?.invoke),
      hasLibraryButton: !!document.querySelector('#btn-library'),
      hasSearchInput: !!document.querySelector('#library-search-input'),
      hasHint: !!document.querySelector('#library-opds-url-hint'),
      hasOpdsPanel: !!document.querySelector('#library-opds-panel'),
      hasOpdsUrlInput: !!document.querySelector('#opds-source-url'),
    }`),
    (value) =>
      value.readyState !== 'loading' &&
      value.hasInvoke &&
      value.hasLibraryButton &&
      value.hasSearchInput &&
      value.hasHint &&
      value.hasOpdsPanel &&
      value.hasOpdsUrlInput,
  )

  const hint = await execute(`
    document.querySelector('#btn-library')?.click();
    const input = document.querySelector('#library-search-input');
    input.value = arguments[0];
    input.dispatchEvent(new Event('input', { bubbles: true }));
    const hint = document.querySelector('#library-opds-url-hint');
    return {
      visible: !!hint && hint.hidden === false,
      text: document.querySelector('#library-opds-url-text')?.textContent || '',
    };
  `, [opdsUrl])
  assertCheck(hint.visible && hint.text === opdsUrl, 'OPDS URL hint did not appear for pasted feed URL', hint)

  const filled = await execute(`
    document.querySelector('#btn-library-opds-use-url')?.click();
    return {
      panelOpen: document.querySelector('#library-opds-panel')?.open === true,
      url: document.querySelector('#opds-source-url')?.value || '',
      name: document.querySelector('#opds-source-name')?.value || '',
      hintHidden: document.querySelector('#library-opds-url-hint')?.hidden === true,
    };
  `)
  assertCheck(
    filled.panelOpen && filled.url === opdsUrl && filled.name && filled.hintHidden,
    'OPDS URL hint did not fill source form',
    filled,
  )

  await execute(`document.querySelector('#btn-opds-add-source')?.click(); return true;`)
  const source = await waitFor(
    'OPDS source added',
    () => invoke('opds_list_sources'),
    (sources) => Array.isArray(sources) && sources.some((item) => item.baseUrl === opdsUrl),
  )
  const addedSource = source.find((item) => item.baseUrl === opdsUrl)
  assertCheck(addedSource?.id, 'added OPDS source is missing id', source)

  await waitFor(
    'OPDS source row rendered',
    () => execute(`
      const rows = [...document.querySelectorAll('.opds-source-row')];
      return rows.map((row) => ({
        text: row.textContent || '',
        buttonCount: row.querySelectorAll('button').length,
      }));
    `),
    (rows) => rows.some((row) => row.text.includes(opdsUrl) && row.buttonCount >= 1),
  )

  await execute(`
    const rows = [...document.querySelectorAll('.opds-source-row')];
    const row = rows.find((item) => (item.textContent || '').includes(arguments[0]));
    const browse = row?.querySelector('button');
    browse?.click();
    return !!browse;
  `, [opdsUrl])
  await waitFor(
    'Gutenberg OPDS search feed rendered',
    () => execute(`
      return {
        hidden: document.querySelector('#opds-feed-view')?.hidden,
        title: document.querySelector('#opds-feed-title')?.textContent || '',
        cards: [...document.querySelectorAll('.opds-feed-card-title')].map((item) => item.textContent || ''),
      };
    `),
    (value) => value.hidden === false && value.cards.some((title) => title.includes(expectedTitle)),
    60_000,
  )

  await execute(`
    const cards = [...document.querySelectorAll('.opds-feed-card')];
    const card = cards.find((item) =>
      (item.querySelector('.opds-feed-card-title')?.textContent || '').includes(arguments[0])
    );
    const button = card?.querySelector('button');
    button?.click();
    return { clicked: !!button, text: card?.textContent || '' };
  `, [expectedTitle])
  await waitFor(
    'Gutenberg OPDS detail feed rendered',
    () => execute(`
      return {
        title: document.querySelector('#opds-feed-title')?.textContent || '',
        cards: [...document.querySelectorAll('.opds-feed-card')].map((card) => ({
          title: card.querySelector('.opds-feed-card-title')?.textContent || '',
          text: card.textContent || '',
          hasEpubButton: [...card.querySelectorAll('button')].some((button) => (button.textContent || '').includes('EPUB')),
        })),
      };
    `),
    (value) =>
      value.title.includes(expectedTitle) &&
      value.cards.some((card) => card.title.includes(expectedTitle) && card.hasEpubButton),
    60_000,
  )

  await execute(`
    const cards = [...document.querySelectorAll('.opds-feed-card')];
    const card = cards.find((item) =>
      (item.querySelector('.opds-feed-card-title')?.textContent || '').includes(arguments[0])
    );
    const button = [...(card?.querySelectorAll('button') || [])].find((item) =>
      (item.textContent || '').includes('EPUB')
    );
    button?.click();
    return { clicked: !!button, text: card?.textContent || '' };
  `, [expectedTitle])

  const downloaded = await waitFor(
    'OPDS EPUB downloaded into local library',
    async () => {
      const books = await invoke('library_list')
      return books.find((book) =>
        (book.title || '').includes(expectedTitle) &&
        Number(book.fileSize || 0) > 0 &&
        book.availability !== 'remote',
      ) || null
    },
    Boolean,
    120_000,
  )

  const opened = await invoke('library_open', { id: downloaded.id })
  const firstHref = opened?.info?.spine?.[0]?.href || opened?.info?.toc?.[0]?.href
  assertCheck(opened?.bookId && firstHref, 'downloaded OPDS book did not open with a readable spine', opened)
  let readableChapter = null
  const hrefs = (opened.info.spine || [])
    .map((item) => item.href)
    .filter(Boolean)
    .slice(0, 12)
  for (const href of hrefs) {
    const chapterHtml = await invoke('get_chapter', { href })
    const chapterText = String(chapterHtml)
      .replace(/<style[\s\S]*?<\/style>/gi, ' ')
      .replace(/<script[\s\S]*?<\/script>/gi, ' ')
      .replace(/<[^>]+>/g, ' ')
      .replace(/\s+/g, ' ')
      .trim()
    if (chapterText.length > 200) {
      readableChapter = { href, textLength: chapterText.length, preview: chapterText.slice(0, 200) }
      break
    }
  }
  assertCheck(
    !!readableChapter,
    'downloaded OPDS chapter did not contain enough readable text',
    { checkedHrefs: hrefs },
  )

  console.log('tauri-opds-smoke: OK')
  console.log(`tauri-opds-smoke: source=${opdsUrl}`)
  console.log(`tauri-opds-smoke: downloaded=${downloaded.title} (${downloaded.id})`)
}

try {
  await main()
} catch (error) {
  console.error('tauri-opds-smoke: FAILED')
  console.error(error?.stack || error?.message || error)
  console.error(`tauri-opds-smoke: appDataDir=${appDataDir}`)
  const ui = sessionId
    ? await execute(`
        return {
          libraryVisible: document.querySelector('#library-view')?.hidden === false,
          hintHidden: document.querySelector('#library-opds-url-hint')?.hidden,
          opdsTitle: document.querySelector('#opds-feed-title')?.textContent || '',
          opdsText: document.querySelector('#opds-feed-view')?.textContent?.slice(0, 800) || '',
          gridText: document.querySelector('#library-grid')?.textContent?.slice(0, 800) || '',
        }
      `).catch(() => null)
    : null
  if (ui) console.error(`tauri-opds-smoke: last UI state=${JSON.stringify(ui)}`)
  const err = tail(stderr)
  const out = tail(stdout)
  if (err) console.error(`tauri-driver stderr:\n${err}`)
  if (out) console.error(`tauri-driver stdout:\n${out}`)
  process.exitCode = 1
} finally {
  await cleanup()
}
