import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { resolve } from 'node:path'
import process from 'node:process'

const args = process.argv.slice(2)

function readOption(name, fallback = '') {
  const eq = args.find((arg) => arg.startsWith(`${name}=`))
  if (eq) return eq.slice(name.length + 1)
  const index = args.indexOf(name)
  return index >= 0 && index + 1 < args.length ? args[index + 1] : fallback
}

function hasFlag(name) {
  return args.includes(name)
}

function fail(message) {
  throw new Error(message)
}

function delay(ms) {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, ms))
}

async function waitForValue(label, producer, predicate, timeoutMs) {
  const started = Date.now()
  let lastValue
  while (Date.now() - started < timeoutMs) {
    lastValue = await producer()
    if (predicate(lastValue)) return lastValue
    await delay(250)
  }
  fail(`${label} timed out. Last value: ${JSON.stringify(lastValue)}`)
}

function runPowerShell(command, extraEnv = {}) {
  return new Promise((resolveCommand, rejectCommand) => {
    const stdout = []
    const stderr = []
    const child = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', command], {
      env: { ...process.env, ...extraEnv },
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
    })
    child.stdout?.on('data', (chunk) => stdout.push(String(chunk)))
    child.stderr?.on('data', (chunk) => stderr.push(String(chunk)))
    child.once('error', rejectCommand)
    child.once('exit', (code) => {
      if (code === 0) resolveCommand(stdout.join('').trim())
      else rejectCommand(new Error(stderr.join('').trim() || `PowerShell exited with ${code}`))
    })
  })
}

const applicationArg = readOption('--application')
const expectedCurrentVersion = readOption('--expected-current-version')
const expectedUpdateVersion = readOption('--expected-update-version')
const devtoolsPort = Number(readOption('--devtools-port', '17778'))
const clearStaleServiceWorker = hasFlag('--clear-stale-service-worker')
const acceptLegacyInstallerLanguage = hasFlag('--accept-legacy-installer-language')

if (process.platform !== 'win32') fail('the updater install smoke currently supports Windows only')
if (!hasFlag('--confirm-install-updater')) fail('missing explicit --confirm-install-updater safety flag')
if (!applicationArg) fail('missing --application')
if (!expectedCurrentVersion) fail('missing --expected-current-version')
if (!expectedUpdateVersion) fail('missing --expected-update-version')
if (!Number.isInteger(devtoolsPort) || devtoolsPort <= 0) fail(`invalid --devtools-port: ${devtoolsPort}`)

const application = resolve(applicationArg)
if (!existsSync(application)) fail(`application not found: ${application}`)

const powershellEnv = { TAURI_SMOKE_APPLICATION: application }
const versionCommand = '(Get-Item -LiteralPath $env:TAURI_SMOKE_APPLICATION).VersionInfo.ProductVersion'

async function readProductVersion() {
  return runPowerShell(versionCommand, powershellEnv)
}

async function acceptInstallerLanguagePrompt() {
  return runPowerShell(
    `$installers = @(Get-Process -ErrorAction SilentlyContinue | Where-Object {
  $_.MainWindowTitle -eq 'Installer Language' -and
  $_.Path -like '*LightNovel Reader-*installer.exe'
})
if ($installers.Count -eq 0) { return }
if ($installers.Count -ne 1) { throw "Expected one installer language prompt, found $($installers.Count)" }
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class UpdaterSmokeKey {
  [DllImport("user32.dll", SetLastError=true)]
  public static extern bool PostMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);
}
'@
$installer = $installers[0]
$down = [UpdaterSmokeKey]::PostMessage($installer.MainWindowHandle, 0x0100, [IntPtr]0x0D, [IntPtr]0)
$up = [UpdaterSmokeKey]::PostMessage($installer.MainWindowHandle, 0x0101, [IntPtr]0x0D, [IntPtr]0)
if (-not ($down -and $up)) { throw 'Failed to accept installer language prompt' }
$installer.Id`,
  )
}

async function closeTestApplication() {
  await runPowerShell(
    `$target = [IO.Path]::GetFullPath($env:TAURI_SMOKE_APPLICATION)
Get-Process reader -ErrorAction SilentlyContinue | Where-Object {
  try { [IO.Path]::GetFullPath($_.Path) -eq $target } catch { $false }
} | ForEach-Object { $null = $_.CloseMainWindow() }
Start-Sleep -Milliseconds 750
Get-Process reader -ErrorAction SilentlyContinue | Where-Object {
  try { [IO.Path]::GetFullPath($_.Path) -eq $target } catch { $false }
} | Stop-Process`,
    powershellEnv,
  ).catch(() => undefined)
}

async function getTargets() {
  try {
    const response = await fetch(`http://127.0.0.1:${devtoolsPort}/json`)
    if (!response.ok) return []
    return await response.json()
  } catch {
    return []
  }
}

async function waitForTarget() {
  const targets = await waitForValue(
    'WebView2 DevTools target',
    getTargets,
    (value) =>
      Array.isArray(value) &&
      value.some(
        (target) =>
          target.type === 'page' &&
          target.url &&
          target.url !== 'about:blank' &&
          target.webSocketDebuggerUrl,
      ),
    30_000,
  )
  return targets.find(
    (target) =>
      target.type === 'page' &&
      target.url &&
      target.url !== 'about:blank' &&
      target.webSocketDebuggerUrl,
  )
}

async function connectCdp(target) {
  const socket = new WebSocket(target.webSocketDebuggerUrl)
  const pending = new Map()
  let nextId = 1
  let closed = false

  await new Promise((resolveOpen, rejectOpen) => {
    socket.addEventListener('open', resolveOpen, { once: true })
    socket.addEventListener('error', rejectOpen, { once: true })
  })

  socket.addEventListener('message', (event) => {
    const message = JSON.parse(String(event.data))
    if (!message.id || !pending.has(message.id)) return
    const { resolveMessage, rejectMessage } = pending.get(message.id)
    pending.delete(message.id)
    if (message.error) rejectMessage(new Error(message.error.message || JSON.stringify(message.error)))
    else resolveMessage(message.result)
  })
  socket.addEventListener('close', () => {
    closed = true
    for (const { rejectMessage } of pending.values()) rejectMessage(new Error('DevTools connection closed'))
    pending.clear()
  })

  function send(method, params = {}) {
    if (closed) return Promise.reject(new Error('DevTools connection closed'))
    const id = nextId
    nextId += 1
    return new Promise((resolveMessage, rejectMessage) => {
      pending.set(id, { resolveMessage, rejectMessage })
      socket.send(JSON.stringify({ id, method, params }))
    })
  }

  async function evaluate(expression) {
    const result = await send('Runtime.evaluate', {
      expression,
      returnByValue: true,
      awaitPromise: true,
    })
    if (result.exceptionDetails) fail(`DevTools evaluation failed: ${JSON.stringify(result.exceptionDetails)}`)
    return result.result?.value
  }

  return { send, evaluate, isClosed: () => closed, close: () => socket.close() }
}

async function inspectServiceWorkerState(connection) {
  return connection.evaluate(`(async () => {
    const registrations = 'serviceWorker' in navigator
      ? await navigator.serviceWorker.getRegistrations()
      : []
    const cacheNames = 'caches' in window ? await caches.keys() : []
    return {
      controller: navigator.serviceWorker?.controller?.scriptURL || '',
      registrations: registrations.map((registration) => ({
        scope: registration.scope,
        active: registration.active?.scriptURL || '',
      })),
      cacheNames,
    }
  })()`)
}

async function clearServiceWorkerState(connection) {
  const result = await connection.evaluate(`(async () => {
    const registrations = 'serviceWorker' in navigator
      ? await navigator.serviceWorker.getRegistrations()
      : []
    const cacheNames = 'caches' in window ? await caches.keys() : []
    await Promise.all(registrations.map((registration) => registration.unregister()))
    if ('caches' in window) {
      await Promise.all(cacheNames.map((cacheName) => caches.delete(cacheName)))
    }
    return {
      controller: navigator.serviceWorker?.controller?.scriptURL || '',
      registrationsRemoved: registrations.length,
      cachesRemoved: cacheNames.length,
    }
  })()`)
  await connection.send('Network.enable')
  await connection.send('Network.clearBrowserCache')
  await connection.send('Storage.clearDataForOrigin', {
    origin: 'http://tauri.localhost',
    storageTypes: 'cache_storage,service_workers',
  })
  return result
}

let directApplication = null
let cdp = null
let serviceWorkerBefore = null
let serviceWorkerRecovery = null
let serviceWorkerAfterUpdate = null
let legacyInstallerLanguagePid = null

const currentWebViewArgs = process.env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS || ''
if (currentWebViewArgs.includes('--remote-debugging-port')) {
  fail('WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS already contains a remote debugging port')
}

function launchApplication() {
  return spawn(application, [], {
    detached: false,
    env: {
      ...process.env,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:
        `${currentWebViewArgs} --remote-debugging-port=${devtoolsPort}`.trim(),
    },
    stdio: 'ignore',
    windowsHide: false,
  })
}

try {
  const initialVersion = await readProductVersion()
  if (initialVersion !== expectedCurrentVersion) {
    fail(`installed version is ${initialVersion}; expected ${expectedCurrentVersion}`)
  }

  directApplication = launchApplication()

  cdp = await connectCdp(await waitForTarget())
  serviceWorkerBefore = await inspectServiceWorkerState(cdp)
  if (clearStaleServiceWorker) {
    serviceWorkerRecovery = await clearServiceWorkerState(cdp)
    cdp.close()
    cdp = null
    await closeTestApplication()
    await delay(1_000)
    directApplication = launchApplication()
    cdp = await connectCdp(await waitForTarget())
  }
  await waitForValue(
    'updater controls',
    () => cdp.evaluate(`(() => {
      const library = document.querySelector('#library-view')
      if (library?.hidden) document.querySelector('#btn-library')?.click()
      return {
        title: document.title,
        readyState: document.readyState,
        hasTauriInternals: !!window.__TAURI_INTERNALS__,
        hasLibraryButton: !!document.querySelector('#btn-library'),
        hasLibraryView: !!document.querySelector('#library-view'),
        hasUpdateControls: !!document.querySelector('#app-update-controls'),
        hasUpdateButton: !!document.querySelector('#btn-app-update'),
        libraryHasUpdateMarkup: document.querySelector('#library-view')?.innerHTML.includes('app-update-controls') === true,
        controlsVisible: document.querySelector('#app-update-controls')?.hidden === false,
        buttonLabel: document.querySelector('#btn-app-update')?.textContent || '',
      }
    })()`),
    (value) => value?.title === '阅读器' && value.controlsVisible && value.buttonLabel === '检查更新',
    20_000,
  )

  await cdp.evaluate(`(() => {
    window.__updaterConfirmMessage = ''
    window.confirm = (message) => {
      window.__updaterConfirmMessage = String(message || '')
      return false
    }
    document.querySelector('#btn-app-update')?.click()
    return true
  })()`)
  const available = await waitForValue(
    'safe updater check',
    () => cdp.evaluate(`(() => ({
      state: document.querySelector('#app-update-controls')?.dataset.state || '',
      status: document.querySelector('#app-update-status')?.textContent || '',
      buttonLabel: document.querySelector('#btn-app-update')?.textContent || '',
      confirmMessage: window.__updaterConfirmMessage || '',
    }))()`),
    (value) => value?.state === 'available' || value?.state === 'error',
    30_000,
  )
  if (available.state === 'error') fail(`updater check failed: ${available.status}`)
  const expectedLabel = expectedUpdateVersion.startsWith('v')
    ? expectedUpdateVersion
    : `v${expectedUpdateVersion}`
  if (!available.status.includes(expectedLabel)) fail(`wrong updater target: ${JSON.stringify(available)}`)
  if (!available.confirmMessage.includes(expectedLabel) || !available.confirmMessage.includes('更新说明')) {
    fail(`updater confirmation is incomplete: ${JSON.stringify(available)}`)
  }

  await cdp.evaluate(`(() => {
    window.__updaterConfirmMessage = ''
    window.confirm = (message) => {
      window.__updaterConfirmMessage = String(message || '')
      return true
    }
    document.querySelector('#btn-app-update')?.click()
    return true
  })()`)

  await waitForValue(
    'old application exit',
    async () => {
      if (cdp.isClosed()) return true
      try {
        await cdp.evaluate('document.title')
        return false
      } catch {
        return true
      }
    },
    (value) => value === true,
    180_000,
  )

  let installedVersion = ''
  if (acceptLegacyInstallerLanguage) {
    const legacyTransition = await waitForValue(
      'legacy installer language prompt or completed install',
      async () => {
        const version = await readProductVersion()
        if (version === expectedUpdateVersion) return { version, promptPid: '' }
        const promptPid = await acceptInstallerLanguagePrompt()
        return { version, promptPid }
      },
      (value) => value.version === expectedUpdateVersion || /^\d+$/.test(value.promptPid),
      30_000,
    )
    installedVersion = legacyTransition.version
    legacyInstallerLanguagePid = legacyTransition.promptPid || null
  }

  if (installedVersion !== expectedUpdateVersion) {
    installedVersion = await waitForValue(
      'installed application version',
      readProductVersion,
      (value) => value === expectedUpdateVersion,
      120_000,
    )
  }

  cdp = await connectCdp(await waitForTarget())
  if (clearStaleServiceWorker) {
    serviceWorkerAfterUpdate = await clearServiceWorkerState(cdp)
    cdp.close()
    cdp = null
    await closeTestApplication()
    await delay(1_000)
    directApplication = launchApplication()
    cdp = await connectCdp(await waitForTarget())
  }
  const relaunchedUi = await waitForValue(
    'relaunched updater UI',
    () => cdp.evaluate(`(() => ({
      title: document.title,
      hasUpdateDialog: !!document.querySelector('#app-update-dialog'),
      hasUpdateProgress: !!document.querySelector('#app-update-progress'),
      hasUpdateButton: !!document.querySelector('#btn-app-update'),
    }))()`),
    (value) => value?.title === '阅读器' && value.hasUpdateDialog && value.hasUpdateProgress && value.hasUpdateButton,
    30_000,
  )
  cdp.close()
  cdp = null

  console.log(JSON.stringify({
    ok: true,
    application,
    initialVersion,
    installedVersion,
    update: available,
    serviceWorkerBefore,
    serviceWorkerRecovery,
    serviceWorkerAfterUpdate,
    legacyInstallerLanguagePid,
    relaunchedUi,
  }, null, 2))
  console.log('tauri-updater-install-smoke: OK')
} catch (error) {
  console.error('tauri-updater-install-smoke: FAILED')
  console.error(error?.stack || error?.message || String(error))
  process.exitCode = 1
} finally {
  cdp?.close()
  await closeTestApplication()
}
