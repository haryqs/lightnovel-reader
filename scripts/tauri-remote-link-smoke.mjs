import { spawn } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
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
const driverPort = Number(readOption('--driver-port', process.env.TAURI_DRIVER_PORT || '4460'))
const nativePort = Number(readOption('--native-port', process.env.TAURI_NATIVE_DRIVER_PORT || '9531'))
const fixturesDir = resolve(
  readOption('--fixtures-dir', process.env.SMOKE_FIXTURES_DIR || join(tmpdir(), 'lightnovel-reader-smoke-epubs')),
)
const remoteSource = readOption('--source', process.env.REMOTE_LINK_SMOKE_SOURCE || 'anilist')
const remoteQuery = readOption('--query', process.env.REMOTE_LINK_SMOKE_QUERY || 'Tanya')
const appDataDir = resolve(
  readOption('--app-data-dir', process.env.LIGHTNOVEL_READER_APP_DATA_DIR || mkdtempSync(join(tmpdir(), 'lnr-remote-link-'))),
)
const autoAppDataDir = !process.env.LIGHTNOVEL_READER_APP_DATA_DIR && !args.some((arg) => arg === '--app-data-dir' || arg.startsWith('--app-data-dir='))
const keepOpen = hasFlag('--keep-open')
const keepData = hasFlag('--keep-data') || !autoAppDataDir
const skipFixtures = hasFlag('--skip-fixtures')
const server = `http://127.0.0.1:${driverPort}`
const vol1 = join(fixturesDir, 'one', 'smoke-test-lightnovel-vol1.epub')

function failPreflight(message) {
  console.error(`tauri-remote-link-smoke: ${message}`)
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

function runPowerShell(script, scriptArgs) {
  return new Promise((resolveRun, rejectRun) => {
    const child = spawn(
      'powershell',
      ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', script, ...scriptArgs],
      { cwd: repoRoot, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true },
    )
    let output = ''
    child.stdout?.on('data', (chunk) => { output += String(chunk) })
    child.stderr?.on('data', (chunk) => { output += String(chunk) })
    child.once('error', rejectRun)
    child.once('exit', (code, signal) => {
      if (code === 0) resolveRun(output)
      else rejectRun(new Error(`PowerShell exited with ${code ?? signal}:\n${output.trim()}`))
    })
  })
}

function runPowerShellInline(command, scriptArgs) {
  return new Promise((resolveRun, rejectRun) => {
    const env = { ...process.env }
    let script = command
    scriptArgs.forEach((arg, index) => {
      const name = `LNR_SMOKE_PS_ARG_${index}`
      env[name] = arg
      script = script.replaceAll(`$args[${index}]`, `$env:${name}`)
    })
    const encoded = Buffer.from(script, 'utf16le').toString('base64')
    const child = spawn(
      'powershell',
      ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-EncodedCommand', encoded],
      { cwd: repoRoot, env, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true },
    )
    let output = ''
    child.stdout?.on('data', (chunk) => { output += String(chunk) })
    child.stderr?.on('data', (chunk) => { output += String(chunk) })
    child.once('error', rejectRun)
    child.once('exit', (code, signal) => {
      if (code === 0) resolveRun(output)
      else rejectRun(new Error(`PowerShell exited with ${code ?? signal}:\n${output.trim()}`))
    })
  })
}

if (!skipFixtures) {
  await runPowerShell(join(repoRoot, 'scripts', 'new-smoke-epubs.ps1'), ['-OutDir', fixturesDir])
}
if (!existsSync(vol1)) failPreflight(`fixture EPUB not found: ${vol1}`)
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

async function execute(script, scriptArgs = []) {
  const payload = await request(`/session/${sessionId}/execute/sync`, {
    method: 'POST',
    body: { script, args: scriptArgs },
  })
  return payload.value
}

async function executeAsync(script, scriptArgs = []) {
  const payload = await request(`/session/${sessionId}/execute/async`, {
    method: 'POST',
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
    await delay(250)
  }
  throw new Error(`${label} timed out. Last value: ${JSON.stringify(lastValue)}`)
}

function assertCheck(condition, message, details) {
  if (!condition) throw new Error(details ? `${message}: ${JSON.stringify(details)}` : message)
}

function xmlEscape(value) {
  return String(value || '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&apos;')
}

async function createLinkFixture(title, author) {
  const safeTitle = title?.trim() || 'Remote Link Smoke Match'
  const safeAuthor = author?.trim() || 'Smoke Tester'
  const buildDir = join(fixturesDir, '_remote_link_match_build')
  const target = join(fixturesDir, 'link', 'remote-link-match.epub')
  rmSync(buildDir, { recursive: true, force: true })
  rmSync(target, { force: true })
  mkdirSync(join(buildDir, 'META-INF'), { recursive: true })
  mkdirSync(join(buildDir, 'OEBPS', 'Text'), { recursive: true })
  mkdirSync(dirname(target), { recursive: true })

  writeFileSync(join(buildDir, 'mimetype'), 'application/epub+zip')
  writeFileSync(join(buildDir, 'META-INF', 'container.xml'), `<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
`)
  writeFileSync(join(buildDir, 'OEBPS', 'content.opf'), `<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="bookid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">remote-link-match-${Date.now()}</dc:identifier>
    <dc:title>${xmlEscape(safeTitle)}</dc:title>
    <dc:creator>${xmlEscape(safeAuthor)}</dc:creator>
    <dc:language>ja</dc:language>
    <dc:description>Temporary EPUB generated for remote-link smoke candidate ranking.</dc:description>
    <meta property="belongs-to-collection" id="series">${xmlEscape(safeTitle)}</meta>
    <meta refines="#series" property="group-position">1</meta>
  </metadata>
  <manifest>
    <item id="chap1" href="Text/chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chap1"/>
  </spine>
</package>
`)
  writeFileSync(join(buildDir, 'OEBPS', 'Text', 'chapter1.xhtml'), `<!doctype html>
<html xmlns="http://www.w3.org/1999/xhtml">
  <head><title>Chapter 1</title></head>
  <body>
    <h1>${xmlEscape(safeTitle)}</h1>
    <p>This local EPUB intentionally matches a remote metadata result so the smoke test can assert candidate scores and reasons.</p>
  </body>
</html>
`)
  await runPowerShellInline(
    "$ErrorActionPreference='Stop'; Add-Type -AssemblyName System.IO.Compression.FileSystem; if (Test-Path -LiteralPath $args[1]) { Remove-Item -LiteralPath $args[1] -Force }; [System.IO.Compression.ZipFile]::CreateFromDirectory($args[0], $args[1])",
    [buildDir, target],
  )
  rmSync(buildDir, { recursive: true, force: true })
  return target
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
    'Tauri invoke ready',
    () => execute('return { readyState: document.readyState, hasInvoke: !!(window.__TAURI__?.core?.invoke || window.__TAURI_INTERNALS__?.invoke) }'),
    (value) => value.readyState !== 'loading' && value.hasInvoke,
  )
  await waitFor(
    'library controls ready',
    () => execute(`return {
      hasLibraryButton: !!document.querySelector('#btn-library'),
      hasSearchInput: !!document.querySelector('#library-search-input'),
      hasRemoteSource: !!document.querySelector('#library-remote-source'),
      hasRemoteSearch: !!document.querySelector('#btn-library-search-remote'),
    }`),
    (value) => value.hasLibraryButton && value.hasSearchInput && value.hasRemoteSource && value.hasRemoteSearch,
  )

  await execute(`
    document.querySelector('#btn-library')?.click();
    const input = document.querySelector('#library-search-input');
    const source = document.querySelector('#library-remote-source');
    input.value = arguments[0];
    input.dispatchEvent(new Event('input', { bubbles: true }));
    source.value = arguments[1];
    source.dispatchEvent(new Event('change', { bubbles: true }));
    window.confirm = () => true;
    document.querySelector('#btn-library-search-remote')?.click();
    return true;
  `, [remoteQuery, remoteSource])

  const before = await waitFor(
    'remote search results',
    () => execute(`
      return Array.from(document.querySelectorAll('#library-grid .book-card')).map((card) => ({
        id: card.dataset.bookId,
        title: card.querySelector('.title')?.textContent || '',
        author: card.querySelector('.author')?.textContent || '',
        remote: card.classList.contains('book-card-remote'),
        hasLink: !!card.querySelector('[data-action="link-remote"]'),
      }));
    `),
    (cards) => cards.some((card) => card.remote && card.hasLink),
    45_000,
  )
  const remoteBefore = before.filter((card) => card.remote)
  let target = remoteBefore[0]
  assertCheck(target?.id, 'remote card is missing data-book-id', before)

  const linkFixture = await createLinkFixture(target.title, target.author)
  const imported = await invoke('library_import', { path: linkFixture })
  const local = imported.book
  assertCheck(local?.id && local?.filePath, 'local matching EPUB did not import as a readable asset', imported)

  const now = Date.now()
  const annotationId = `remote-link-smoke-${now}`
  await invoke('save_progress', {
    progress: {
      bookId: local.id,
      chapterHref: 'Text/chapter1.xhtml',
      chapterProgress: 0.42,
      percentage: 0.42,
      updatedAt: now,
    },
  })
  await invoke('save_annotation', {
    annotation: {
      id: annotationId,
      bookId: local.id,
      kind: 'highlight',
      color: 'yellow',
      note: 'remote link smoke',
      locator: {
        chapterHref: 'Text/chapter1.xhtml',
        anchor: {
          start: 0,
          end: 5,
          exact: 'This ',
          prefix: '',
          suffix: 'local EPUB',
        },
      },
      createdAt: now,
      updatedAt: now,
    },
  })

  await execute(`
    const button = document.querySelector('#library-grid .book-card-remote [data-action="link-remote"]');
    button?.click();
    return !!button;
  `)
  await waitFor(
    'local link candidate panel',
    () => execute('return document.querySelectorAll(".library-link-candidate").length'),
    (count) => count >= 1,
  )
  const candidatePanel = await execute(`
    const matches = Array.from(document.querySelectorAll('.library-link-candidate-match')).map((el) => el.textContent || '');
    const scores = matches.map((text) => Number((text.match(/匹配\\s*(\\d+)/) || [])[1])).filter((n) => Number.isFinite(n));
    return {
      title: document.querySelector('.library-link-title')?.textContent || '',
      subtitle: document.querySelector('.library-link-subtitle')?.textContent || '',
      matches,
      scores,
      warnings: Array.from(document.querySelectorAll('.library-link-candidate-warning')).map((el) => el.textContent || ''),
    };
  `)
  assertCheck(candidatePanel.title === '关联本地书', 'local link candidate panel title mismatch', candidatePanel)
  assertCheck(candidatePanel.scores.length >= 1, 'candidate panel did not show match scores', candidatePanel)
  assertCheck(candidatePanel.matches.some((text) => /匹配\s*\d+\s*·/.test(text)), 'candidate panel did not show match reasons', candidatePanel)
  assertCheck(
    candidatePanel.scores.every((score, index, scores) => index === 0 || scores[index - 1] >= score),
    'candidate scores are not sorted descending',
    candidatePanel,
  )
  await execute(`
    document.querySelector('.library-link-panel .icon-btn')?.click();
    return !document.querySelector('.library-link-panel');
  `)

  await execute(`
    const button = document.querySelector('#btn-library-batch-link');
    button?.click();
    return { clicked: !!button, disabled: button?.disabled };
  `)
  const batchPanel = await waitFor(
    'batch link panel',
    () => execute(`
      const rows = Array.from(document.querySelectorAll('.library-batch-row')).map((row) => ({
        title: row.querySelector('.library-batch-remote-title')?.textContent || '',
        status: row.className,
        selected: row.querySelector('.library-batch-select option:checked')?.textContent || '',
        match: row.querySelector('.library-link-candidate-match')?.textContent || '',
        warning: row.querySelector('.library-link-candidate-warning')?.textContent || '',
      }));
      return {
        title: document.querySelector('.library-batch-panel .library-link-title')?.textContent || '',
        subtitle: document.querySelector('.library-batch-panel .library-link-subtitle')?.textContent || '',
        rows,
      };
    `),
    (value) => value.title === '批量人工确认' && value.rows.length >= 1,
  )
  assertCheck(batchPanel.rows.some((row) => /^\d+\s*·/.test(row.selected)), 'batch queue did not show candidate score in selector', batchPanel)
  assertCheck(batchPanel.rows.some((row) => /匹配\s*\d+/.test(row.match)), 'batch queue did not show candidate match score', batchPanel)
  assertCheck(batchPanel.rows.some((row) => /·/.test(row.match)), 'batch queue did not show candidate match reason', batchPanel)

  await execute(`
    const row = document.querySelector('.library-batch-row');
    const button = row?.querySelector('.library-batch-actions .btn-subtle');
    button?.click();
    return { clicked: !!button, title: row?.querySelector('.library-batch-remote-title')?.textContent || '' };
  `)
  await waitFor(
    'batch skip row',
    () => execute(`
      return {
        subtitle: document.querySelector('.library-batch-panel .library-link-subtitle')?.textContent || '',
        skipped: Array.from(document.querySelectorAll('.library-batch-row-skipped .library-batch-status')).map((el) => el.textContent || ''),
        pending: document.querySelectorAll('.library-batch-row:not(.library-batch-row-skipped):not(.library-batch-row-linked)').length,
      };
    `),
    (value) => value.skipped.some((text) => text.includes('已跳过')) && value.subtitle.includes('已跳过'),
  )

  let linkedTitle = await execute(`
    const row = document.querySelector('.library-batch-row:not(.library-batch-row-skipped):not(.library-batch-row-linked)');
    const title = row?.querySelector('.library-batch-remote-title')?.textContent || '';
    const button = row?.querySelector('.library-batch-actions .btn-primary');
    button?.click();
    return title;
  `)
  if (!linkedTitle) {
    await execute(`
      document.querySelector('.library-batch-panel .icon-btn')?.click();
      document.querySelector('#btn-library-batch-link')?.click();
      return true;
    `)
    await waitFor(
      'batch link panel reopened',
      () => execute('return document.querySelectorAll(".library-batch-row").length'),
      (count) => count >= 1,
    )
    linkedTitle = await execute(`
      const row = document.querySelector('.library-batch-row');
      const title = row?.querySelector('.library-batch-remote-title')?.textContent || '';
      const button = row?.querySelector('.library-batch-actions .btn-primary');
      button?.click();
      return title;
    `)
  }
  target = remoteBefore.find((card) => card.title === linkedTitle) || target

  const batchLinked = await waitFor(
    'batch link row',
    () => execute(`
      return {
        linked: Array.from(document.querySelectorAll('.library-batch-row-linked .library-batch-status')).map((el) => el.textContent || ''),
        cards: Array.from(document.querySelectorAll('#library-grid .book-card')).map((card) => ({
          id: card.dataset.bookId,
          title: card.querySelector('.title')?.textContent || '',
          remote: card.classList.contains('book-card-remote'),
        })),
      };
    `),
    (value) => value.linked.some((text) => text.includes('已关联')) && !value.cards.some((card) => card.id === target.id),
    20_000,
  )

  await execute(`
    document.querySelector('.library-batch-footer .btn')?.click();
    return true;
  `)

  const after = await waitFor(
    'remote card linked away',
    () => execute(`
      return {
        summary: document.querySelector('.library-import-summary')?.textContent || '',
        cards: Array.from(document.querySelectorAll('#library-grid .book-card')).map((card) => ({
          id: card.dataset.bookId,
          title: card.querySelector('.title')?.textContent || '',
          remote: card.classList.contains('book-card-remote'),
        })),
      };
    `),
    (value) => value.cards.some((card) => card.id === local.id) && !value.cards.some((card) => card.id === target.id),
    20_000,
  )

  await execute(`
    const card = Array.from(document.querySelectorAll('#library-grid .book-card')).find((item) => item.dataset.bookId === arguments[0]);
    const button = card?.querySelector('[data-action="show-sources"]');
    button?.click();
    return !!button;
  `, [local.id])
  const sourcePanel = await waitFor(
    'source record panel',
    () => execute(`
      return {
        title: document.querySelector('.library-source-panel .library-link-title')?.textContent || '',
        subtitle: document.querySelector('.library-source-panel .library-link-subtitle')?.textContent || '',
        records: Array.from(document.querySelectorAll('.library-source-record')).map((row) => ({
          name: row.querySelector('.library-source-record-title')?.textContent || '',
          meta: row.querySelector('.library-source-record-meta')?.textContent || '',
          url: row.querySelector('.library-source-record-url')?.textContent || '',
        })),
      };
    `),
    (value) => value.title === '来源记录' && value.records.length >= 1,
  )
  assertCheck(sourcePanel.subtitle.includes(local.title), 'source panel did not open for linked local book', sourcePanel)
  assertCheck(sourcePanel.records.some((record) => record.name && record.meta && record.url.startsWith('http')), 'source panel did not show source metadata and URL', sourcePanel)

  const listAfterLink = await invoke('library_list')
  assertCheck(listAfterLink.some((book) => book.id === local.id), 'local book missing after link', listAfterLink)
  assertCheck(!listAfterLink.some((book) => book.id === target.id), 'linked remote shell still visible in library list', {
    target,
    listAfterLink,
  })
  const progress = await invoke('get_progress', { bookId: local.id })
  assertCheck(progress?.bookId === local.id && Math.abs(progress.percentage - 0.42) < 0.001, 'reading progress key changed after link', progress)
  const annotations = await invoke('list_annotations', { bookId: local.id })
  assertCheck(annotations.some((item) => item.id === annotationId), 'annotation key changed after link', annotations)

  await execute(`
    const input = document.querySelector('#library-search-input');
    const source = document.querySelector('#library-remote-source');
    input.value = arguments[0];
    input.dispatchEvent(new Event('input', { bubbles: true }));
    source.value = arguments[1];
    source.dispatchEvent(new Event('change', { bubbles: true }));
    document.querySelector('#btn-library-search-remote')?.click();
    return true;
  `, [remoteQuery, remoteSource])
  await waitFor(
    'repeat remote search finished',
    () => execute('return document.querySelector("#btn-library-search-remote")?.disabled === false'),
    (done) => done === true,
    45_000,
  )
  const listAfterRepeat = await invoke('library_list')
  assertCheck(!listAfterRepeat.some((book) => book.id === target.id), 'repeat remote search made the orphan shell visible again', {
    target,
    listAfterRepeat,
  })

  console.log(JSON.stringify({
    ok: true,
    source: remoteSource,
    query: remoteQuery,
    local: { id: local.id, title: local.title },
    linkedRemote: target,
    candidatePanel: { scores: candidatePanel.scores, matches: candidatePanel.matches.slice(0, 3) },
    batchLinked: batchLinked.linked,
    sourceRecords: sourcePanel.records.map((record) => record.name),
    cardsAfterLink: after.cards.length,
    annotations: annotations.length,
  }, null, 2))
  console.log('tauri-remote-link-smoke: OK')
}

try {
  await main()
} catch (error) {
  console.error('tauri-remote-link-smoke: FAILED')
  console.error(error?.message || error)
  console.error('driver:', tail(stderr) || tail(stdout))
  if (keepData) console.error(`app data kept at: ${appDataDir}`)
  process.exitCode = 1
} finally {
  await cleanup()
}
