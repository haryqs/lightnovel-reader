import { spawn } from 'node:child_process'
import { existsSync, readdirSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
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
  const root = join(localAppData, 'lightnovel-reader-tools', 'msedgedriver')
  if (!existsSync(root)) return ''
  const versions = readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && /^\d+(?:\.\d+){3}$/.test(entry.name))
    .map((entry) => entry.name)
    .sort((left, right) => {
      const a = left.split('.').map(Number)
      const b = right.split('.').map(Number)
      for (let i = 0; i < Math.max(a.length, b.length); i += 1) {
        if ((a[i] || 0) !== (b[i] || 0)) return (b[i] || 0) - (a[i] || 0)
      }
      return 0
    })
  for (const version of versions) {
    const candidate = join(root, version, 'msedgedriver.exe')
    if (existsSync(candidate)) return candidate
  }
  return ''
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
const keepOpen = hasFlag('--keep-open')
const checkUpdater = hasFlag('--check-updater')
const server = `http://127.0.0.1:${driverPort}`

function failPreflight(message) {
  console.error(`tauri-webdriver-smoke: ${message}`)
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
  if (target.length > 80) target.splice(0, target.length - 80)
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

async function execute(script, scriptArgs = []) {
  const payload = await request(`/session/${sessionId}/execute/sync`, {
    method: 'POST',
    body: { script, args: scriptArgs },
  })
  return payload.value
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

function assertCheck(condition, message, details) {
  if (!condition) {
    throw new Error(details ? `${message}: ${JSON.stringify(details)}` : message)
  }
}

async function cleanup() {
  if (keepOpen) return
  if (sessionId) {
    try {
      await request(`/session/${sessionId}`, { method: 'DELETE', timeoutMs: 5_000 })
    } catch {
      // Best effort cleanup. The driver process is still terminated below.
    }
  }
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
  sessionId = sessionIdFrom(session)
  assertCheck(sessionId, 'WebDriver did not return a session id', session)

  const boot = await waitForValue(
    'initial DOM',
    () =>
      execute(`
        const ambient = document.querySelector('.ambient-scene .ambient-landscape')
        const emptyArt = document.querySelector('.empty-art-frame img')
        return {
          title: document.title,
          theme: document.body?.dataset.theme || '',
          hasBrand: !!document.querySelector('.app-brand img'),
          hasAmbientScene: !!ambient,
          hasEmptyArt: !!emptyArt,
          ambientLoaded: !!ambient && ambient.complete && ambient.naturalWidth > 0,
          emptyArtLoaded: !!emptyArt && emptyArt.complete && emptyArt.naturalWidth > 0,
          hasEmptyOpen: !!document.querySelector('#btn-empty-open'),
          hasEmptyLibrary: !!document.querySelector('#btn-empty-library'),
          hasOpenButton: !!document.querySelector('#btn-open'),
          hasLibraryButton: !!document.querySelector('#btn-library'),
          hasLibraryView: !!document.querySelector('#library-view'),
          libraryInitiallyHidden: document.querySelector('#library-view')?.hidden === true,
        }
      `),
    (value) =>
      value.hasOpenButton &&
      value.hasLibraryButton &&
      value.hasLibraryView &&
      value.ambientLoaded &&
      value.emptyArtLoaded,
  )

  assertCheck(boot.title === '阅读器', 'unexpected application title', boot)
  assertCheck(boot.hasBrand, 'application brand mark is missing', boot)
  assertCheck(
    boot.hasAmbientScene && boot.hasEmptyArt && boot.ambientLoaded && boot.emptyArtLoaded,
    'illustration layers are missing or failed to load',
    boot,
  )
  assertCheck(boot.hasEmptyOpen && boot.hasEmptyLibrary, 'empty-state actions are missing', boot)
  assertCheck(boot.libraryInitiallyHidden, 'library overlay should be hidden on boot', boot)

  await execute(`
    localStorage.removeItem('reader-theme')
    location.reload()
    return true
  `)

  const defaultTheme = await waitForValue(
    'default theme after clearing persisted preference',
    () =>
      execute(`
        return {
          title: document.title,
          theme: document.body?.dataset.theme || '',
          hasBrand: !!document.querySelector('.app-brand img'),
          hasLibraryButton: !!document.querySelector('#btn-library'),
        }
      `),
    (value) => value.title === '阅读器' && value.hasBrand && value.hasLibraryButton,
  )
  assertCheck(
    defaultTheme.theme === 'light',
    'default theme should be light for the anime-style shelf refresh',
    defaultTheme,
  )

  await execute(`document.querySelector('#btn-library')?.click(); return true`)

  const library = await waitForValue(
    'library overlay',
    () =>
      execute(`
        const view = document.querySelector('#library-view')
        const sourcePanel = document.querySelector('#library-source-panel')
        const calibre = document.querySelector('#btn-library-import-calibre')
        const updateControls = document.querySelector('#app-update-controls')
        const updateButton = document.querySelector('#btn-app-update')
        const updateProgress = document.querySelector('#app-update-progress')
        return {
          libraryVisible: !!view && view.hidden === false,
          hasImportEpub: !!document.querySelector('#btn-library-import-epub'),
          hasImportFolder: !!document.querySelector('#btn-library-import-folder'),
          hasSearch: !!document.querySelector('#library-search-input'),
          hasGrid: !!document.querySelector('#library-grid'),
          sourcePanelOpen: sourcePanel?.open === true,
          calibreInsideSourcePanel: !!sourcePanel && !!calibre && sourcePanel.contains(calibre),
          hasUpdateButton: !!updateButton,
          updateControlsVisible: !!updateControls && updateControls.hidden === false,
          updateButtonLabel: updateButton?.textContent || '',
          hasUpdateProgress: !!updateProgress,
          updateProgressHidden: updateProgress?.hidden === true,
          bookCards: document.querySelectorAll('.book-card').length,
          hasEmptyState: !!document.querySelector('.library-empty'),
          hasErrorState: !!document.querySelector('.library-state-error'),
        }
      `),
    (value) => value.libraryVisible && value.hasGrid,
  )

  assertCheck(library.hasImportEpub, 'library import EPUB action is missing', library)
  assertCheck(library.hasImportFolder, 'library folder import action is missing', library)
  assertCheck(library.hasSearch, 'library search input is missing', library)
  assertCheck(library.calibreInsideSourcePanel, 'Calibre migration must stay under the secondary source panel', library)
  assertCheck(!library.sourcePanelOpen, 'secondary source panel should be collapsed by default', library)
  assertCheck(!library.hasErrorState, 'library rendered an error state', library)
  assertCheck(library.hasUpdateButton, 'application update action is missing', library)
  assertCheck(library.hasUpdateProgress, 'application update progress indicator is missing', library)
  assertCheck(library.updateProgressHidden, 'application update progress should be hidden before installation', library)
  assertCheck(library.updateControlsVisible, 'application update action must be visible in Tauri', library)
  assertCheck(library.updateButtonLabel === '检查更新', 'unexpected application update label', library)

  let updater = null
  if (checkUpdater) {
    await execute(`
      window.confirm = () => false
      document.querySelector('#btn-app-update')?.click()
      return true
    `)
    updater = await waitForValue(
      'application updater check',
      () => execute(`
        const controls = document.querySelector('#app-update-controls')
        const progress = document.querySelector('#app-update-progress')
        return {
          state: controls?.dataset.state || '',
          status: document.querySelector('#app-update-status')?.textContent || '',
          buttonLabel: document.querySelector('#btn-app-update')?.textContent || '',
          progressHidden: progress?.hidden === true,
        }
      `),
      (value) => value.state === 'success' || value.state === 'available' || value.state === 'error',
      30_000,
    )
    assertCheck(updater.state !== 'error', 'application updater check failed', updater)
    assertCheck(updater.progressHidden, 'updater check must not show install progress', updater)
  }

  const closed = await execute(`
    document.querySelector('#btn-library-close')?.click()
    return document.querySelector('#library-view')?.hidden === true
  `)
  assertCheck(closed === true, 'library close button did not hide the overlay')

  console.log(
    JSON.stringify(
      {
        ok: true,
        application,
        nativeDriver,
        sessionId,
        checks: { boot, defaultTheme, library, updater, closed },
        keepOpen,
      },
      null,
      2,
    ),
  )
  console.log('tauri-webdriver-smoke: OK')
}

try {
  await main()
} catch (error) {
  console.error('tauri-webdriver-smoke: FAILED')
  console.error(error?.stack || error?.message || String(error))
  const err = tail(stderr)
  const out = tail(stdout)
  if (err) console.error(`tauri-driver stderr:\n${err}`)
  if (out) console.error(`tauri-driver stdout:\n${out}`)
  process.exitCode = 1
} finally {
  await cleanup()
}
