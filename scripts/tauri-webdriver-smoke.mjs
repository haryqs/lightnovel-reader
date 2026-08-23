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
const legacyUpdaterUi = hasFlag('--legacy-updater-ui')
const expectedUpdateVersion = readOption('--expected-update-version', '')
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
if (hasFlag('--install-updater')) {
  failPreflight('--install-updater is not supported by WebDriver; use scripts/tauri-updater-install-smoke.mjs')
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
        const updateDialog = document.querySelector('#app-update-dialog')
        const backupControls = document.querySelector('#library-backup-controls')
        const backupButton = document.querySelector('#btn-library-backup')
        const backupInspectButton = document.querySelector('#btn-library-backup-inspect')
        const backupStatus = document.querySelector('#library-backup-status')
        const backupInspectionDialog = document.querySelector('#backup-inspection-dialog')
        const libraryFilter = document.querySelector('#library-filter')
        const librarySort = document.querySelector('#library-sort')
        const resultSummary = document.querySelector('#library-result-summary')
        return {
          libraryVisible: !!view && view.hidden === false,
          hasImportEpub: !!document.querySelector('#btn-library-import-epub'),
          hasImportFolder: !!document.querySelector('#btn-library-import-folder'),
          hasSearch: !!document.querySelector('#library-search-input'),
          hasLibraryFilter: !!libraryFilter,
          libraryFilterOptions: Array.from(libraryFilter?.options || []).map((option) => option.value),
          hasLibrarySort: !!librarySort,
          librarySortOptions: Array.from(librarySort?.options || []).map((option) => option.value),
          hasLibraryResultSummary: !!resultSummary,
          libraryResultSummary: resultSummary?.textContent || '',
          libraryOrganizeEnabled: libraryFilter?.disabled === false && librarySort?.disabled === false,
          hasGrid: !!document.querySelector('#library-grid'),
          sourcePanelOpen: sourcePanel?.open === true,
          calibreInsideSourcePanel: !!sourcePanel && !!calibre && sourcePanel.contains(calibre),
          hasUpdateButton: !!updateButton,
          updateControlsVisible: !!updateControls && updateControls.hidden === false,
          updateButtonLabel: updateButton?.textContent || '',
          hasUpdateProgress: !!updateProgress,
          updateProgressHidden: updateProgress?.hidden === true,
          hasUpdateDialog: !!updateDialog,
          updateDialogClosed: updateDialog?.open === false,
          backupControlsVisible: !!backupControls && backupControls.hidden === false,
          backupButtonLabel: backupButton?.textContent || '',
          backupButtonEnabled: backupButton?.disabled === false,
          backupInspectButtonLabel: backupInspectButton?.textContent || '',
          backupInspectButtonEnabled: backupInspectButton?.disabled === false,
          hasBackupInspectionDialog: !!backupInspectionDialog,
          backupInspectionDialogClosed: backupInspectionDialog?.open === false,
          hasBackupStatus: !!backupStatus,
          bookCards: document.querySelectorAll('.book-card').length,
          hasEmptyState: !!document.querySelector('.library-empty'),
          hasErrorState: !!document.querySelector('.library-state-error'),
        }
      `),
    (value) =>
      value.libraryVisible &&
      value.hasGrid &&
      (value.bookCards > 0 || value.hasEmptyState || value.hasErrorState),
  )

  assertCheck(library.hasImportEpub, 'library import EPUB action is missing', library)
  assertCheck(library.hasImportFolder, 'library folder import action is missing', library)
  assertCheck(library.hasSearch, 'library search input is missing', library)
  assertCheck(library.hasLibraryFilter, 'library filter is missing', library)
  assertCheck(
    library.libraryFilterOptions.join(',') === 'all,readable,remote,unread',
    'library filter options are incomplete',
    library,
  )
  assertCheck(library.hasLibrarySort, 'library sort is missing', library)
  assertCheck(
    library.librarySortOptions.join(',') === 'recent,added,title,author',
    'library sort options are incomplete',
    library,
  )
  assertCheck(library.hasLibraryResultSummary, 'library result summary is missing', library)
  assertCheck(library.libraryResultSummary.trim().length > 0, 'library result summary is empty', library)
  assertCheck(library.libraryOrganizeEnabled, 'library organize controls should be enabled for the shelf', library)
  assertCheck(library.calibreInsideSourcePanel, 'Calibre migration must stay under the secondary source panel', library)
  assertCheck(!library.sourcePanelOpen, 'secondary source panel should be collapsed by default', library)
  assertCheck(!library.hasErrorState, 'library rendered an error state', library)
  assertCheck(library.hasUpdateButton, 'application update action is missing', library)
  if (!legacyUpdaterUi) {
    assertCheck(library.hasUpdateProgress, 'application update progress indicator is missing', library)
    assertCheck(library.updateProgressHidden, 'application update progress should be hidden before installation', library)
    assertCheck(library.hasUpdateDialog, 'application update details dialog is missing', library)
    assertCheck(library.updateDialogClosed, 'application update details dialog should be closed initially', library)
  }
  assertCheck(library.updateControlsVisible, 'application update action must be visible in Tauri', library)
  assertCheck(library.updateButtonLabel === '检查更新', 'unexpected application update label', library)
  assertCheck(library.backupControlsVisible, 'user data backup action must be visible in Tauri', library)
  assertCheck(library.backupButtonLabel === '备份数据', 'unexpected user data backup label', library)
  assertCheck(library.backupButtonEnabled, 'user data backup action should be enabled initially', library)
  assertCheck(library.backupInspectButtonLabel === '校验备份', 'unexpected backup inspection label', library)
  assertCheck(library.backupInspectButtonEnabled, 'backup inspection action should be enabled initially', library)
  assertCheck(library.hasBackupInspectionDialog, 'backup inspection dialog is missing', library)
  assertCheck(library.backupInspectionDialogClosed, 'backup inspection dialog should be closed initially', library)
  assertCheck(library.hasBackupStatus, 'user data backup status region is missing', library)

  const libraryOrganizeProbe = await execute(`
    const filter = document.querySelector('#library-filter')
    const sort = document.querySelector('#library-sort')
    filter.value = 'unread'
    filter.dispatchEvent(new Event('change', { bubbles: true }))
    sort.value = 'title'
    sort.dispatchEvent(new Event('change', { bubbles: true }))
    const result = {
      filterValue: filter.value,
      sortValue: sort.value,
      summary: document.querySelector('#library-result-summary')?.textContent || '',
      hasShelfState: !!document.querySelector('.library-empty, .library-state, .book-card'),
    }
    filter.value = 'all'
    filter.dispatchEvent(new Event('change', { bubbles: true }))
    sort.value = 'recent'
    sort.dispatchEvent(new Event('change', { bubbles: true }))
    return result
  `)
  assertCheck(libraryOrganizeProbe.filterValue === 'unread', 'library filter change was not applied', libraryOrganizeProbe)
  assertCheck(libraryOrganizeProbe.sortValue === 'title', 'library sort change was not applied', libraryOrganizeProbe)
  assertCheck(libraryOrganizeProbe.summary.trim().length > 0, 'library organize summary disappeared', libraryOrganizeProbe)
  assertCheck(libraryOrganizeProbe.hasShelfState, 'library organize change cleared the shelf state', libraryOrganizeProbe)

  let updateDialogProbe = null
  let updateDialogDismissed = null
  if (!legacyUpdaterUi) {
    updateDialogProbe = await execute(`
      const dialog = document.querySelector('#app-update-dialog')
      dialog.returnValue = ''
      dialog.showModal()
      return {
        open: dialog.open,
        labelledBy: dialog.getAttribute('aria-labelledby') || '',
        describedBy: dialog.getAttribute('aria-describedby') || '',
        hasLater: !!document.querySelector('#btn-app-update-later'),
        hasInstall: !!document.querySelector('#btn-app-update-install'),
        cardTag: dialog.querySelector('.app-update-dialog-card')?.tagName || '',
        laterType: document.querySelector('#btn-app-update-later')?.type || '',
      }
    `)
    assertCheck(updateDialogProbe.open, 'application update details dialog did not open', updateDialogProbe)
    assertCheck(updateDialogProbe.labelledBy === 'app-update-dialog-title', 'update dialog label is missing', updateDialogProbe)
    assertCheck(updateDialogProbe.describedBy === 'app-update-dialog-description', 'update dialog description is missing', updateDialogProbe)
    assertCheck(updateDialogProbe.hasLater && updateDialogProbe.hasInstall, 'update dialog actions are missing', updateDialogProbe)
    assertCheck(updateDialogProbe.cardTag === 'DIV', 'update dialog must not rely on form submission', updateDialogProbe)
    assertCheck(updateDialogProbe.laterType === 'button', 'update dialog later action must be an explicit button', updateDialogProbe)
    await execute(`document.querySelector('#btn-app-update-later')?.click(); return true`)
    updateDialogDismissed = await waitForValue(
      'dismiss update details dialog',
      () => execute(`return document.querySelector('#app-update-dialog')?.open === false`),
      (value) => value === true,
    )
  }

  const backupInspectionDialogProbe = await execute(`
    const dialog = document.querySelector('#backup-inspection-dialog')
    dialog.returnValue = ''
    dialog.showModal()
    return {
      open: dialog.open,
      labelledBy: dialog.getAttribute('aria-labelledby') || '',
      describedBy: dialog.getAttribute('aria-describedby') || '',
      summaryItems: dialog.querySelectorAll('.backup-inspection-summary > div').length,
      hasReadOnlyCopy: (document.querySelector('#backup-inspection-dialog-description')?.textContent || '').includes('不会替换当前数据'),
      hasRestorePlanStatus: !!document.querySelector('#backup-restore-plan-status'),
      prepareLabel: document.querySelector('#btn-backup-restore-prepare')?.textContent || '',
      prepareType: document.querySelector('#btn-backup-restore-prepare')?.type || '',
      prepareDisabledWithoutPlan: document.querySelector('#btn-backup-restore-prepare')?.disabled === true,
      title: document.querySelector('#backup-inspection-dialog-title')?.textContent || '',
      closeType: document.querySelector('#btn-backup-inspection-close')?.type || '',
    }
  `)
  assertCheck(backupInspectionDialogProbe.open, 'backup inspection dialog did not open', backupInspectionDialogProbe)
  assertCheck(backupInspectionDialogProbe.labelledBy === 'backup-inspection-dialog-title', 'backup inspection dialog label is missing', backupInspectionDialogProbe)
  assertCheck(backupInspectionDialogProbe.describedBy === 'backup-inspection-dialog-description', 'backup inspection dialog description is missing', backupInspectionDialogProbe)
  assertCheck(backupInspectionDialogProbe.summaryItems === 8, 'backup inspection summary is incomplete', backupInspectionDialogProbe)
  assertCheck(backupInspectionDialogProbe.hasReadOnlyCopy, 'backup inspection safety copy is missing', backupInspectionDialogProbe)
  assertCheck(backupInspectionDialogProbe.hasRestorePlanStatus, 'restore plan status is missing', backupInspectionDialogProbe)
  assertCheck(backupInspectionDialogProbe.prepareLabel.includes('不恢复'), 'restore preparation safety label is missing', backupInspectionDialogProbe)
  assertCheck(backupInspectionDialogProbe.prepareType === 'button', 'restore preparation must be an explicit button', backupInspectionDialogProbe)
  assertCheck(backupInspectionDialogProbe.prepareDisabledWithoutPlan, 'restore preparation must stay disabled without a plan', backupInspectionDialogProbe)
  assertCheck(backupInspectionDialogProbe.title === '备份校验与恢复计划', 'restore plan title is wrong', backupInspectionDialogProbe)
  assertCheck(backupInspectionDialogProbe.closeType === 'button', 'backup inspection close action must be explicit', backupInspectionDialogProbe)
  await execute(`document.querySelector('#btn-backup-inspection-close')?.click(); return true`)
  await waitForValue(
    'dismiss backup inspection dialog',
    () => execute(`return document.querySelector('#backup-inspection-dialog')?.open === false`),
    (value) => value === true,
  )

  let updater = null
  if (checkUpdater) {
    if (legacyUpdaterUi) {
      await execute(`
        window.__updaterConfirmMessage = ''
        window.confirm = (message) => {
          window.__updaterConfirmMessage = String(message || '')
          return false
        }
        return true
      `)
    }
    await execute(`
      document.querySelector('#btn-app-update')?.click()
      return true
    `)
    updater = await waitForValue(
      'application updater check',
      () => execute(`
        const controls = document.querySelector('#app-update-controls')
        const progress = document.querySelector('#app-update-progress')
        const dialog = document.querySelector('#app-update-dialog')
        return {
          state: controls?.dataset.state || '',
          status: document.querySelector('#app-update-status')?.textContent || '',
          buttonLabel: document.querySelector('#btn-app-update')?.textContent || '',
          progressHidden: progress?.hidden === true,
          dialogOpen: dialog?.open === true,
          currentVersion: document.querySelector('#app-update-current-version')?.textContent || '',
          targetVersion: document.querySelector('#app-update-target-version')?.textContent || '',
          releaseNotes: document.querySelector('#app-update-release-notes-body')?.textContent || '',
          confirmMessage: window.__updaterConfirmMessage || '',
        }
      `),
      (value) =>
        value.state === 'success' ||
        value.state === 'error' ||
        (value.state === 'available' && (legacyUpdaterUi || value.dialogOpen)),
      30_000,
    )
    assertCheck(updater.state !== 'error', 'application updater check failed', updater)
    if (!legacyUpdaterUi) {
      assertCheck(updater.progressHidden, 'updater check must not show install progress', updater)
    }
    if (expectedUpdateVersion) {
      const expectedLabel = expectedUpdateVersion.startsWith('v')
        ? expectedUpdateVersion
        : `v${expectedUpdateVersion}`
      assertCheck(updater.state === 'available', `expected update ${expectedLabel} was not available`, updater)
      if (legacyUpdaterUi) {
        assertCheck(updater.status.includes(expectedLabel), 'legacy updater status has the wrong target version', updater)
        assertCheck(updater.confirmMessage.includes(expectedLabel), 'legacy updater confirmation has the wrong target version', updater)
        assertCheck(updater.confirmMessage.includes('更新说明'), 'legacy updater confirmation is missing release notes', updater)
      } else {
        assertCheck(updater.targetVersion === expectedLabel, 'updater dialog has the wrong target version', updater)
      }
    }
    if (updater.state === 'available') {
      if (legacyUpdaterUi) {
        assertCheck(updater.confirmMessage.trim().length > 0, 'legacy updater confirmation was not shown', updater)
        updater.cancelledSafely = true
      } else {
        assertCheck(updater.dialogOpen, 'available update must open the details dialog', updater)
        assertCheck(updater.currentVersion.startsWith('v'), 'update dialog current version is missing', updater)
        assertCheck(updater.targetVersion.startsWith('v'), 'update dialog target version is missing', updater)
        assertCheck(updater.releaseNotes.trim().length > 0, 'update dialog release notes are missing', updater)
        await execute(`document.querySelector('#btn-app-update-later')?.click(); return true`)
        const cancelled = await waitForValue(
          'cancel application update safely',
          () => execute(`
            return {
              dialogOpen: document.querySelector('#app-update-dialog')?.open === true,
              state: document.querySelector('#app-update-controls')?.dataset.state || '',
              buttonLabel: document.querySelector('#btn-app-update')?.textContent || '',
            }
          `),
          (value) => !value.dialogOpen,
        )
        assertCheck(cancelled.state === 'available', 'cancelling update must preserve the available state', cancelled)
        assertCheck(cancelled.buttonLabel === '安装更新', 'cancelling update must keep the install action', cancelled)
        updater.cancelledSafely = true
      }
    }

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
        checks: {
          boot,
          defaultTheme,
          library,
          libraryOrganizeProbe,
          updateDialogProbe,
          updateDialogDismissed,
          backupInspectionDialogProbe,
          updater,
          closed,
        },
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
