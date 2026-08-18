import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve, sep } from 'node:path'
import { spawnSync } from 'node:child_process'

const workDir = mkdtempSync(join(tmpdir(), 'lnr-updater-release-test-'))
const bundleDir = join(workDir, 'bundle')
const nsisDir = join(bundleDir, 'nsis')
const outDir = join(workDir, 'out')
const configPath = join(workDir, 'tauri.conf.json')
const sourceName = 'LightNovel Reader_0.7.0_x64-setup.exe'
const assetName = 'LightNovel.Reader_0.7.0_x64-setup.exe'

function assertCheck(condition, message) {
  if (!condition) throw new Error(message)
}

function cleanup() {
  const resolvedTmp = resolve(tmpdir())
  const resolvedWork = resolve(workDir)
  if (!resolvedWork.startsWith(`${resolvedTmp}${sep}`)) {
    throw new Error(`refusing to remove path outside the system temp directory: ${resolvedWork}`)
  }
  rmSync(resolvedWork, { recursive: true, force: true })
}

try {
  mkdirSync(nsisDir, { recursive: true })
  writeFileSync(join(nsisDir, sourceName), 'signed installer fixture')
  writeFileSync(join(nsisDir, `${sourceName}.sig`), 'signed updater fixture\n')
  writeFileSync(configPath, `${JSON.stringify({ version: '0.7.0' })}\n`)

  const result = spawnSync(process.execPath, [
    join(import.meta.dirname, 'prepare-updater-release.mjs'),
    '--bundle-dir', bundleDir,
    '--config', configPath,
    '--out-dir', outDir,
    '--base-url', 'https://github.com/haryqs/lightnovel-reader/releases/download/v0.7.0',
    '--pub-date', '2026-08-04T00:00:00.000Z',
  ], { encoding: 'utf8', windowsHide: true })

  assertCheck(result.status === 0, result.stderr || result.stdout || 'prepare updater fixture failed')
  assertCheck(existsSync(join(outDir, assetName)), 'GitHub-safe installer name should be emitted')
  assertCheck(existsSync(join(outDir, `${assetName}.sig`)), 'signature should follow the emitted installer name')
  assertCheck(!existsSync(join(outDir, sourceName)), 'space-containing installer name should not be emitted')

  const latest = JSON.parse(readFileSync(join(outDir, 'latest.json'), 'utf8'))
  assertCheck(latest.version === '0.7.0', 'latest.json should preserve the configured version')
  assertCheck(
    latest.platforms['windows-x86_64'].url ===
      `https://github.com/haryqs/lightnovel-reader/releases/download/v0.7.0/${assetName}`,
    'latest.json should reference the GitHub-safe installer asset',
  )
  assertCheck(
    latest.platforms['windows-x86_64'].signature === 'signed updater fixture',
    'latest.json should embed the updater signature',
  )

  console.log('test-prepare-updater-release: OK')
} finally {
  cleanup()
}
