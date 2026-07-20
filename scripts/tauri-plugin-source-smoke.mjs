import { spawn } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve, sep } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const args = process.argv.slice(2)

function readOption(name, fallback) {
  const inline = args.find((arg) => arg.startsWith(`${name}=`))
  if (inline) return inline.slice(name.length + 1)
  const index = args.indexOf(name)
  return index >= 0 && index + 1 < args.length ? args[index + 1] : fallback
}

function hasFlag(name) {
  return args.includes(name)
}

function defaultNativeDriver() {
  if (!process.env.LOCALAPPDATA) return ''
  const root = join(process.env.LOCALAPPDATA, 'lightnovel-reader-tools', 'msedgedriver')
  if (!existsSync(root)) return ''
  const versions = readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort((left, right) => right.localeCompare(left, undefined, { numeric: true }))
  return versions
    .map((version) => join(root, version, 'msedgedriver.exe'))
    .find((candidate) => existsSync(candidate)) || ''
}

const driverBinary = readOption('--tauri-driver', process.env.TAURI_DRIVER || 'tauri-driver')
const nativeDriver = resolve(readOption('--native-driver', process.env.MSEDGEDRIVER || defaultNativeDriver()))
const application = resolve(
  readOption(
    '--application',
    process.env.TAURI_APP_PATH || join(repoRoot, 'target', 'debug', process.platform === 'win32' ? 'reader.exe' : 'reader'),
  ),
)
const pluginPackage = resolve(
  readOption('--plugin-package', join(repoRoot, 'scripts', 'test-plugin', 'test-plugin-hello.zip')),
)
const driverPort = Number(readOption('--driver-port', process.env.TAURI_DRIVER_PORT || '4448'))
const nativePort = Number(readOption('--native-port', process.env.TAURI_NATIVE_DRIVER_PORT || '9519'))
const customAppDataDir = readOption('--app-data-dir', process.env.LIGHTNOVEL_READER_APP_DATA_DIR || '')
const appDataDir = customAppDataDir
  ? resolve(customAppDataDir)
  : mkdtempSync(join(tmpdir(), 'lightnovel-reader-plugin-source-smoke-'))
const autoAppDataDir = !customAppDataDir
const keepData = hasFlag('--keep-data') || !autoAppDataDir
const keepOpen = hasFlag('--keep-open')
const webdriverServer = `http://127.0.0.1:${driverPort}`
const pluginId = 'test-plugin-hello'

function failPreflight(message) {
  console.error(`tauri-plugin-source-smoke: ${message}`)
  process.exit(1)
}

if (!Number.isInteger(driverPort) || driverPort <= 0) failPreflight(`invalid --driver-port: ${driverPort}`)
if (!Number.isInteger(nativePort) || nativePort <= 0) failPreflight(`invalid --native-port: ${nativePort}`)
if (!existsSync(nativeDriver)) failPreflight(`native WebDriver not found: ${nativeDriver}`)
if (!existsSync(application)) {
  failPreflight(`Tauri debug app not found: ${application}\nBuild it first with: npm.cmd run tauri -- build --debug --no-bundle`)
}
if (!existsSync(pluginPackage)) failPreflight(`offline plugin package not found: ${pluginPackage}`)
mkdirSync(appDataDir, { recursive: true })

const stdout = []
const stderr = []
let driverExited = false
let driverExitCode = null
let driverExitSignal = null
let sessionId = null

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

function assertCheck(condition, message, details) {
  if (!condition) throw new Error(details ? `${message}: ${JSON.stringify(details)}` : message)
}

const driver = spawn(
  driverBinary,
  ['--native-driver', nativeDriver, '--port', String(driverPort), '--native-port', String(nativePort)],
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

async function request(path, { method = 'GET', body, timeoutMs = 10_000 } = {}) {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), timeoutMs)
  try {
    const response = await fetch(`${webdriverServer}${path}`, {
      method,
      headers: body ? { 'content-type': 'application/json' } : undefined,
      body: body ? JSON.stringify(body) : undefined,
      signal: controller.signal,
    })
    const raw = await response.text()
    const payload = raw ? JSON.parse(raw) : {}
    if (!response.ok || payload?.value?.error) {
      throw new Error(`${method} ${path} failed: ${raw || response.status}`)
    }
    return payload
  } catch (error) {
    if (error?.name === 'AbortError') {
      throw new Error(`${method} ${path} timed out after ${timeoutMs}ms`)
    }
    throw error
  } finally {
    clearTimeout(timeout)
  }
}

async function waitForDriver() {
  for (let attempt = 0; attempt < 80; attempt += 1) {
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
  throw new Error(`tauri-driver did not become ready on ${webdriverServer}\n${tail(stderr) || tail(stdout)}`)
}

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
  sessionId = session?.value?.sessionId || session?.sessionId || null
  assertCheck(sessionId, 'WebDriver did not return a session id', session)
}

async function deleteSession() {
  if (!sessionId) return
  const deleting = sessionId
  sessionId = null
  try {
    await request(`/session/${deleting}`, { method: 'DELETE', timeoutMs: 5_000 })
  } catch {
    // Best effort. The driver process is terminated during final cleanup.
  }
  await delay(500)
}

async function execute(script, scriptArgs = []) {
  const response = await request(`/session/${sessionId}/execute/sync`, {
    method: 'POST',
    body: { script, args: scriptArgs },
  })
  return response.value
}

async function executeAsync(script, scriptArgs = [], timeoutMs = 30_000) {
  const response = await request(`/session/${sessionId}/execute/async`, {
    method: 'POST',
    timeoutMs,
    body: { script, args: scriptArgs },
  })
  return response.value
}

async function invoke(command, params = {}, timeoutMs = 30_000) {
  const result = await executeAsync(
    `
      const command = arguments[0]
      const params = arguments[1] || {}
      const done = arguments[arguments.length - 1]
      ;(async () => {
        const invoke = window.__TAURI__?.core?.invoke || window.__TAURI_INTERNALS__?.invoke
        if (!invoke) throw new Error('Tauri invoke is not available')
        done({ ok: true, value: await invoke(command, params) })
      })().catch((error) => done({
        ok: false,
        message: error?.message || String(error),
        value: error,
      }))
    `,
    [command, params],
    timeoutMs,
  )
  if (!result?.ok) throw new Error(`${command} failed: ${result?.message || JSON.stringify(result?.value || result)}`)
  return result.value
}

async function waitForValue(label, producer, predicate, timeoutMs = 15_000) {
  const startedAt = Date.now()
  let lastValue
  while (Date.now() - startedAt < timeoutMs) {
    lastValue = await producer()
    if (predicate(lastValue)) return lastValue
    await delay(200)
  }
  throw new Error(`${label} timed out. Last value: ${JSON.stringify(lastValue)}`)
}

async function waitForInvokeReady(label) {
  return waitForValue(
    label,
    () => execute(`return { readyState: document.readyState, hasInvoke: !!(window.__TAURI__?.core?.invoke || window.__TAURI_INTERNALS__?.invoke) }`),
    (value) => value.hasInvoke && value.readyState !== 'loading',
  )
}

async function uiSnapshot(selector) {
  return execute(
    `
      const element = document.querySelector(arguments[0])
      if (!element) return null
      return {
        text: element.textContent || '',
        hidden: !!element.hidden,
        html: element.innerHTML,
        count: document.querySelectorAll(arguments[0]).length,
      }
    `,
    [selector],
  )
}

async function click(selector, text) {
  const clicked = await execute(
    `
      const candidates = Array.from(document.querySelectorAll(arguments[0]))
      const element = arguments[1]
        ? candidates.find((item) => (item.textContent || '').trim() === arguments[1])
        : candidates[0]
      if (!element) return false
      element.click()
      return true
    `,
    [selector, text || ''],
  )
  assertCheck(clicked, `UI element not found: ${selector}${text ? ` (${text})` : ''}`)
}

async function openLibraryAndSelectSource() {
  await click('#btn-library')
  await waitForValue(
    'plugin source dropdown option',
    () => execute(`return Array.from(document.querySelectorAll('#library-remote-source option')).map((option) => ({ value: option.value, text: option.textContent || '' }))`),
    (options) => options.some((option) => option.value === `plugin:${pluginId}`),
  )
  await execute(
    `
      const input = document.querySelector('#library-search-input')
      const select = document.querySelector('#library-remote-source')
      input.value = '离线冒烟'
      input.dispatchEvent(new Event('input', { bubbles: true }))
      select.value = arguments[0]
      select.dispatchEvent(new Event('change', { bubbles: true }))
    `,
    [`plugin:${pluginId}`],
  )
}

async function runFirstSession() {
  await createSession()
  const boot = await waitForInvokeReady('first Tauri session')
  const installed = await invoke('plugin_install_package', {
    path: pluginPackage,
    confirmUserLegal: true,
  })
  assertCheck(installed.manifest?.id === pluginId && installed.enabled === true, 'offline plugin was not installed enabled', installed)

  await openLibraryAndSelectSource()
  await click('#btn-library-search-remote')
  const searchCard = await waitForValue(
    'plugin search result card',
    () => uiSnapshot('.plugin-source-card'),
    (snapshot) => snapshot?.count === 2 && snapshot.text.includes('测试小说：离线冒烟'),
  )
  const beforeCollect = await invoke('library_list')
  assertCheck(beforeCollect.length === 0, 'plugin search must not write into the library', beforeCollect)

  await click('.plugin-source-card .book-card-actions button', '查看章节')
  const detail = await waitForValue(
    'plugin book detail',
    () => execute(`return { text: document.querySelector('.plugin-source-book')?.textContent || '', chapters: document.querySelectorAll('.plugin-source-chapter-row').length }`),
    (value) => value.chapters === 3 && value.text.includes('测试小说详情'),
  )

  await click('.plugin-source-chapter-row button', '预览正文')
  const preview = await waitForValue(
    'plain-text chapter preview',
    () => execute(`
      const panel = document.querySelector('.plugin-source-chapter-preview')
      const text = panel?.querySelector('.plugin-source-chapter-text')?.textContent || ''
      return {
        text,
        note: panel?.querySelector('.plugin-source-book-meta')?.textContent || '',
        unsafeElements: panel?.querySelectorAll('script, iframe, img, link, style').length || 0,
      }
    `),
    (value) => value.text.includes('完整插件流程工作正常') && value.note.includes('纯文本预览'),
  )
  assertCheck(preview.unsafeElements === 0, 'chapter preview rendered plugin HTML/resources directly', preview)

  await click('.plugin-source-chapter-preview button', '← 返回章节列表')
  await waitForValue('book detail after preview', () => uiSnapshot('.plugin-source-book-toolbar'), (value) => value?.text.includes('收藏来源'))
  await click('.plugin-source-book-toolbar button', '收藏来源')
  await waitForValue(
    'plugin collection summary',
    () => uiSnapshot('.library-import-summary'),
    (value) => value?.text.includes('已收藏插件来源：测试小说详情'),
  )

  const books = await invoke('library_list')
  assertCheck(books.length === 1, 'collection should create exactly one library entry', books)
  const collected = books[0]
  assertCheck(collected.title === '测试小说详情', 'unexpected collected title', collected)
  assertCheck(collected.availability === 'remote' && collected.rightsStatus === 'official_free', 'collection legal/availability mapping mismatch', collected)
  assertCheck(collected.remoteUrl === 'https://example.com/books/book-1', 'collection source URL mismatch', collected)
  assertCheck(!collected.filePath && !collected.acquisitionUrl, 'collection must not acquire or cache content', collected)

  const records = await invoke('library_source_records', { bookId: collected.id })
  assertCheck(records.length === 1, 'collection should create one source record', records)
  assertCheck(records[0].sourceId === `plugin:${pluginId}` && records[0].sourceKind === 'plugin', 'source record does not identify plugin source', records)

  const disabled = await invoke('plugin_set_enabled', { pluginId, enabled: false })
  assertCheck(disabled.enabled === false, 'plugin disable did not persist', disabled)
  await click('#btn-library')
  const optionsAfterDisable = await waitForValue(
    'disabled source removed from dropdown',
    () => execute(`return Array.from(document.querySelectorAll('#library-remote-source option')).map((option) => option.value)`),
    (options) => !options.includes(`plugin:${pluginId}`),
  )
  const sourcesAfterDisable = await invoke('source_list')
  assertCheck(sourcesAfterDisable.length === 0, 'disabled plugin leaked through source.list', sourcesAfterDisable)
  let disabledSearchError = null
  try {
    await invoke('source_search', { pluginId, query: 'blocked', page: 1 })
  } catch (error) {
    disabledSearchError = error
  }
  assertCheck(disabledSearchError, 'disabled plugin source.search should fail')

  await deleteSession()
  return { boot, searchCard: searchCard.text, detail, preview, collected, records, optionsAfterDisable }
}

async function runRestartSession(expectedBookId) {
  await createSession()
  const boot = await waitForInvokeReady('restarted Tauri session')
  const installed = await invoke('plugin_list_installed')
  const plugin = installed.find((item) => item.manifest?.id === pluginId)
  assertCheck(plugin?.enabled === false, 'disabled plugin state did not survive restart', installed)
  const sources = await invoke('source_list')
  assertCheck(sources.length === 0, 'disabled source became visible after restart', sources)
  const books = await invoke('library_list')
  assertCheck(books.length === 1 && books[0].id === expectedBookId, 'collected source did not survive restart', books)
  const enabled = await invoke('plugin_set_enabled', { pluginId, enabled: true })
  assertCheck(enabled.enabled === true, 'plugin could not be re-enabled after restart', enabled)
  const enabledSources = await invoke('source_list')
  assertCheck(enabledSources.length === 1 && enabledSources[0].id === pluginId, 're-enabled source is not available', enabledSources)
  return { boot, installedCount: installed.length, collectedBookId: books[0].id, enabledSources }
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
    const temporaryRoot = resolve(tmpdir())
    const resolvedData = resolve(appDataDir)
    if (resolvedData.startsWith(`${temporaryRoot}${sep}`)) {
      try {
        rmSync(resolvedData, { recursive: true, force: true, maxRetries: 8, retryDelay: 250 })
      } catch (error) {
        console.error(`tauri-plugin-source-smoke: could not remove temporary app data: ${error?.message || error}`)
      }
    }
  }
}

async function main() {
  await waitForDriver()
  const first = await runFirstSession()
  const restarted = await runRestartSession(first.collected.id)
  console.log(JSON.stringify({
    ok: true,
    application,
    nativeDriver,
    pluginPackage,
    appDataDir,
    keepData,
    first,
    restarted,
  }, null, 2))
  console.log('tauri-plugin-source-smoke: OK')
}

try {
  await main()
} catch (error) {
  console.error('tauri-plugin-source-smoke: FAILED')
  console.error(error?.stack || error?.message || String(error))
  if (tail(stderr)) console.error(`tauri-driver stderr:\n${tail(stderr)}`)
  if (tail(stdout)) console.error(`tauri-driver stdout:\n${tail(stdout)}`)
  process.exitCode = 1
} finally {
  if (!keepOpen) await cleanup()
  else console.error(`tauri-plugin-source-smoke: keeping app/data open at ${appDataDir}`)
}
