import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'
import { checkOpenSourceLicense } from './check-open-source-license.mjs'

const scriptDir = dirname(fileURLToPath(import.meta.url))
const repoRoot = resolve(scriptDir, '..')
const workDir = mkdtempSync(join(tmpdir(), 'lnr-open-source-license-test-'))
const manifests = [
  'src-tauri/Cargo.toml',
  'crates/reading-core/Cargo.toml',
  'crates/sync-server/Cargo.toml',
]

function assertCheck(condition, message, details) {
  if (!condition) {
    throw new Error(details ? `${message}: ${JSON.stringify(details)}` : message)
  }
}

function writeValidFixture() {
  writeFileSync(join(workDir, 'LICENSE'), readFileSync(join(repoRoot, 'LICENSE')))
  writeFileSync(
    join(workDir, 'package.json'),
    `${JSON.stringify({ name: 'fixture', license: 'AGPL-3.0-only' }, null, 2)}\n`,
  )
  writeFileSync(
    join(workDir, 'package-lock.json'),
    `${JSON.stringify({ packages: { '': { license: 'AGPL-3.0-only' } } }, null, 2)}\n`,
  )
  for (const relativePath of manifests) {
    const target = join(workDir, relativePath)
    mkdirSync(dirname(target), { recursive: true })
    writeFileSync(target, '[package]\nname = "fixture"\nlicense = "AGPL-3.0-only"\n')
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
  writeValidFixture()
  const ready = checkOpenSourceLicense(workDir)
  assertCheck(ready.ok, 'canonical license fixture should pass', ready)

  writeFileSync(join(workDir, 'LICENSE'), 'not a license\n')
  const changedLicense = checkOpenSourceLicense(workDir)
  assertCheck(!changedLicense.ok, 'modified license text should fail', changedLicense)
  assertCheck(
    changedLicense.errors.some((error) => error.includes('unmodified canonical')),
    'modified license text should have a clear error',
    changedLicense,
  )

  writeValidFixture()
  writeFileSync(join(workDir, 'package.json'), '{"license":"MIT"}\n')
  writeFileSync(join(workDir, 'package-lock.json'), '{"packages":{"":{"license":"MIT"}}}\n')
  writeFileSync(join(workDir, manifests[0]), '[package]\nlicense = "MIT"\n')
  const metadataDrift = checkOpenSourceLicense(workDir)
  assertCheck(!metadataDrift.ok, 'package metadata drift should fail', metadataDrift)
  assertCheck(metadataDrift.errors.length === 3, 'all metadata drift should be reported', metadataDrift)

  console.log('test-open-source-license: OK')
} finally {
  cleanup()
}
