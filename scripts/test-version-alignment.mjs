import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve, sep } from 'node:path'
import { checkVersionAlignment } from './check-version-alignment.mjs'

const workDir = mkdtempSync(join(tmpdir(), 'lnr-version-alignment-test-'))
const cargoManifests = [
  'src-tauri/Cargo.toml',
  'crates/reading-core/Cargo.toml',
  'crates/sync-server/Cargo.toml',
]

function assertCheck(condition, message, details) {
  if (!condition) throw new Error(details ? `${message}: ${JSON.stringify(details)}` : message)
}

function writeFixture({ product = '0.7.0', npm = product, lock = product, cargo = product } = {}) {
  mkdirSync(join(workDir, 'src-tauri'), { recursive: true })
  writeFileSync(join(workDir, 'src-tauri/tauri.conf.json'), `${JSON.stringify({ version: product })}\n`)
  writeFileSync(join(workDir, 'package.json'), `${JSON.stringify({ version: npm })}\n`)
  writeFileSync(
    join(workDir, 'package-lock.json'),
    `${JSON.stringify({ version: lock, packages: { '': { version: lock } } })}\n`,
  )
  for (const relativePath of cargoManifests) {
    const target = join(workDir, relativePath)
    mkdirSync(dirname(target), { recursive: true })
    writeFileSync(target, `[package]\nname = "fixture"\nversion = "${cargo}"\n`)
  }
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
  writeFixture()
  const aligned = checkVersionAlignment(workDir)
  assertCheck(aligned.ok, 'aligned versions should pass', aligned)

  writeFixture({ npm: '0.6.0', lock: '0.5.0', cargo: '0.4.0' })
  const drifted = checkVersionAlignment(workDir)
  assertCheck(!drifted.ok, 'drifted versions should fail', drifted)
  assertCheck(drifted.errors.length === 6, 'all package version drift should be reported', drifted)

  writeFixture({ product: 'v0.7' })
  const invalidProduct = checkVersionAlignment(workDir)
  assertCheck(!invalidProduct.ok, 'non-SemVer product version should fail', invalidProduct)

  console.log('test-version-alignment: OK')
} finally {
  cleanup()
}
