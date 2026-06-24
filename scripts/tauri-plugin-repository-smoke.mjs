import { spawn } from 'node:child_process'
import { existsSync, mkdtempSync, rmSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { tmpdir } from 'node:os'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

// Real Tauri window smoke for the official plugin repository install flow.
//
// It loads a frozen official index over real HTTPS, verifies the candidate
// plugin, confirms install, and checks the installed list refreshes — all
// through the library “源插件（v0.7 预览）” UI, never executing plugin JS.
//
// The fixture is served from a public gist (gist.githubusercontent.com, a
// GitHub cert that rustls/webpki trusts) so the smoke works while the repo
// itself stays private. The canonical fixture also lives in-repo at
// smoke-fixtures/plugin-repository/ and is mirrored to that gist; see the
// fixture README for how to regenerate/mirror.
//
// Build the app first:  npm.cmd run tauri -- build --debug --no-bundle
// Then run:             npm.cmd run smoke:plugin-repository

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
const repositoryUrl = readOption(
  '--repository-url',
  process.env.PLUGIN_REPOSITORY_SMOKE_URL ||
    'https://gist.githubusercontent.com/haryqs/a20cbbeecfb11a744b2650c776f0b615/raw/repository.json',
)
const expectedName = readOption('--name', process.env.PLUGIN_REPOSITORY_SMOKE_NAME || 'Aozora Smoke Source')
const expectedPluginId = readOption('--plugin-id', process.env.PLUGIN_REPOSITORY_SMOKE_PLUGIN_ID || 'aozora-smoke-source')
const appDataDir = resolve(
  readOption('--app-data-dir', process.env.LIGHTNOVEL_READER_APP_DATA_DIR || mkdtempSync(join(tmpdir(), 'lnr-plugin-repository-smoke-'))),
)
const autoAppDataDir =
  !process.env.LIGHTNOVEL_READER_APP_DATA_DIR && !args.some((arg) => arg === '--app-data-dir' || arg.startsWith('--app-data-dir='))
const keepOpen = hasFlag('--keep-open')
const keepData = hasFlag('--keep-data') || !autoAppDataDir
const server = `http://127.0.0.1:${driverPort}`

function failPreflight(message) {
  console.error(`tauri-plugin-repository-smoke: ${message}`)
  process.exit(1)
}

for (const [label, value] of [
  ['driver port', driverPort],
  ['native port', nativePort],
]) {
  if (!Number.isInteger(value) || value <= 0) failPreflight(`invalid ${label}: ${value}`)
}
if (!existsSync(nativeDriver)) failPreflight(`native WebDriver not found: ${nativeDriver}`)
if (!existsSync(application)) {
  failPreflight(
    `Tauri debug app not found: ${application}\n` + 'Build it first with: npm.cmd run tauri -- build --debug --no-bundle',
  )
}

const stdout = []
const stderr = []
let driverExited = false
let driverExitCode = null
let driverExitSignal = null

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

async function snapshotUi() {
  return execute(`return {
    libraryHidden: document.querySelector('#library-view')?.hidden,
    pluginPanelOpen: document.querySelector('#library-plugin-panel')?.open,
    repositoryListText: document.querySelector('#plugin-repository-list')?.textContent?.slice(0, 800) || '',
    repositoryListError: document.querySelector('#plugin-repository-list .plugin-message-error')?.textContent || '',
    previewHidden: document.querySelector('#plugin-install-preview')?.hidden,
    previewTitle: document.querySelector('#plugin-install-preview .plugin-preview-title')?.textContent || '',
    previewError: document.querySelector('#plugin-install-preview .plugin-message-error')?.textContent || '',
    installedRows: [...document.querySelectorAll('#plugin-installed-list .plugin-installed-row')].map((row) => ({
      name: row.querySelector('.plugin-installed-name')?.textContent || '',
      meta: row.querySelector('.plugin-installed-meta')?.textContent || '',
    })),
    installedIds: (window.__lastInstalledIds || null),
  }`).catch(() => null)
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
        const killer = spawn('taskkill', ['/PID', String(driver.pid), '/T', '/F'], { stdio: 'ignore', windowsHide: true })
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

// Click an action button and wait for a success predicate. raw.githubusercontent
// CDN propagation (especially right after a push) can surface as a transient
// network/http error in the UI; when that happens we re-arm the button and try
// again instead of failing the whole smoke.
async function performNetworkAction(label, arm, produce, isSuccess, attempts = 6, stepTimeoutMs = 25_000) {
  let lastValue
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    await arm()
    try {
      lastValue = await waitFor(`${label} (attempt ${attempt})`, produce, isSuccess, stepTimeoutMs)
      return lastValue
    } catch (error) {
      if (attempt === attempts) throw error
      // Transient failure (likely CDN/http error shown in the UI) — back off and retry.
      await delay(2500)
    }
  }
  throw new Error(`${label} failed. Last value: ${JSON.stringify(lastValue)}`)
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
    'plugin repository controls ready',
    () =>
      execute(`return {
        readyState: document.readyState,
        hasInvoke: !!(window.__TAURI__?.core?.invoke || window.__TAURI_INTERNALS__?.invoke),
        hasLibraryButton: !!document.querySelector('#btn-library'),
        hasPluginPanel: !!document.querySelector('#library-plugin-panel'),
        hasRepositoryUrlInput: !!document.querySelector('#plugin-repository-url'),
        hasLoadButton: !!document.querySelector('#btn-plugin-repository-load'),
      }`),
    (value) =>
      value.readyState !== 'loading' &&
      value.hasInvoke &&
      value.hasLibraryButton &&
      value.hasPluginPanel &&
      value.hasRepositoryUrlInput &&
      value.hasLoadButton,
  )

  await execute(`
    if (document.querySelector('#library-view')?.hidden !== false) document.querySelector('#btn-library')?.click();
    document.querySelector('#library-plugin-panel').open = true;
    return true;
  `)
  await waitFor(
    'library view + plugin panel open',
    () =>
      execute(`return {
        libraryHidden: document.querySelector('#library-view')?.hidden,
        pluginPanelOpen: document.querySelector('#library-plugin-panel')?.open,
      }`),
    (value) => value.libraryHidden === false && value.pluginPanelOpen === true,
  )

  // 1) Load the official index over real HTTPS.
  await performNetworkAction(
    'load official plugin index',
    async () => {
      await execute(`
        const input = document.querySelector('#plugin-repository-url');
        input.value = arguments[0];
        input.dispatchEvent(new Event('input', { bubbles: true }));
        document.querySelector('#btn-plugin-repository-load')?.click();
        return true;
      `, [repositoryUrl])
    },
    () =>
      execute(`return {
        error: document.querySelector('#plugin-repository-list .plugin-message-error')?.textContent || '',
        rows: [...document.querySelectorAll('#plugin-repository-list .plugin-repository-row')].map((row) => ({
          name: row.querySelector('.plugin-installed-name')?.textContent || '',
          meta: row.querySelector('.plugin-installed-meta')?.textContent || '',
          hasInspect: !!row.querySelector('button.btn-primary'),
        })),
      }`),
    (value) =>
      !value.error &&
      value.rows.some(
        (row) => row.name.includes(expectedName) && row.meta.includes(expectedPluginId) && row.hasInspect,
      ),
  )

  // 2) Inspect (校验包) the candidate package: downloads zip, verifies SHA-256,
  //    returns the install preview without executing plugin JS.
  await performNetworkAction(
    'inspect official plugin package',
    async () => {
      await execute(`
        const rows = [...document.querySelectorAll('#plugin-repository-list .plugin-repository-row')];
        const row = rows.find((item) => (item.querySelector('.plugin-installed-name')?.textContent || '').includes(arguments[0]));
        const inspect = row?.querySelector('button.btn-primary');
        if (inspect && inspect.disabled === false) inspect.click();
        return !!inspect;
      `, [expectedName])
    },
    () =>
      execute(`return {
        hidden: document.querySelector('#plugin-install-preview')?.hidden,
        title: document.querySelector('#plugin-install-preview .plugin-preview-title')?.textContent || '',
        id: document.querySelector('#plugin-install-preview .plugin-preview-id')?.textContent || '',
        facts: [...document.querySelectorAll('#plugin-install-preview .plugin-preview-fact')].map((fact) => ({
          key: fact.querySelector('span')?.textContent || '',
          value: fact.querySelector('strong')?.textContent || '',
        })),
        error: document.querySelector('#plugin-install-preview .plugin-message-error')?.textContent || '',
      }`),
    (value) =>
      value.hidden === false &&
      !value.error &&
      value.title.includes(expectedName) &&
      value.id.includes(expectedPluginId) &&
      value.facts.some((fact) => fact.key === '域名' && fact.value.includes('example.org')),
  )

  // 3) Confirm install (确认安装). The fixture is public-domain (officially
  //    whitelisted), so no user-declared confirmation checkbox is required.
  await performNetworkAction(
    'install official plugin package',
    async () => {
      await execute(`
        const install = document.querySelector('#plugin-install-preview .plugin-preview-actions button.btn-primary');
        if (install && install.disabled === false) install.click();
        return !!install;
      `)
    },
    async () => {
      const installed = await invoke('plugin_list_installed')
      return {
        installed,
        uiRows: await execute(`return [...document.querySelectorAll('#plugin-installed-list .plugin-installed-row')].map((row) => ({
          name: row.querySelector('.plugin-installed-name')?.textContent || '',
          meta: row.querySelector('.plugin-installed-meta')?.textContent || '',
          badge: row.querySelector('.plugin-badge')?.textContent || '',
        }))`),
      }
    },
    (value) =>
      Array.isArray(value.installed) &&
      value.installed.some((plugin) => plugin?.manifest?.id === expectedPluginId) &&
      value.uiRows.some(
        (row) => row.name.includes(expectedName) && row.meta.includes('已启用') && row.badge.includes('可白名单'),
      ),
    6,
    30_000,
  )

  const installed = await invoke('plugin_list_installed')
  const target = installed.find((plugin) => plugin?.manifest?.id === expectedPluginId)
  assertCheck(!!target, 'installed plugin list missing the smoke plugin', installed)

  console.log('tauri-plugin-repository-smoke: OK')
  console.log(`tauri-plugin-repository-smoke: repository=${repositoryUrl}`)
  console.log(
    `tauri-plugin-repository-smoke: installed=${target.manifest.name} ${target.manifest.version} (${target.manifest.id}, enabled=${target.enabled})`,
  )
}

try {
  await main()
} catch (error) {
  console.error('tauri-plugin-repository-smoke: FAILED')
  console.error(error?.stack || error?.message || error)
  console.error(`tauri-plugin-repository-smoke: appDataDir=${appDataDir}`)
  console.error(`tauri-plugin-repository-smoke: repositoryUrl=${repositoryUrl}`)
  const ui = sessionId ? await snapshotUi().catch(() => null) : null
  if (ui) console.error(`tauri-plugin-repository-smoke: last UI state=${JSON.stringify(ui)}`)
  const err = tail(stderr)
  const out = tail(stdout)
  if (err) console.error(`tauri-driver stderr:\n${err}`)
  if (out) console.error(`tauri-driver stdout:\n${out}`)
  process.exitCode = 1
} finally {
  await cleanup()
}
