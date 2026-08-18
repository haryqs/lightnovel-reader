import { createHash, generateKeyPairSync, sign } from 'node:crypto'
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve, sep } from 'node:path'
import { verifyReleaseCandidate } from './verify-release-candidate.mjs'

const workDir = mkdtempSync(join(tmpdir(), 'lnr-release-candidate-test-'))
const candidateDir = join(workDir, 'candidate')
const configPath = join(workDir, 'tauri.conf.json')
const pluginTrustPath = join(workDir, 'plugin_trust.rs')
const version = '0.7.0'
const releaseBaseUrl = `https://github.com/haryqs/lightnovel-reader/releases/download/v${version}`
const installerName = `LightNovel.Reader_${version}_x64-setup.exe`
const pluginName = 'gutenberg.zip'

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

function assertCheck(condition, message, details) {
  if (!condition) throw new Error(details ? `${message}: ${JSON.stringify(details)}` : message)
}

const { privateKey, publicKey } = generateKeyPairSync('ed25519')
const publicKeyBytes = publicKey.export({ format: 'der', type: 'spki' }).subarray(-32)
const pluginBytes = Buffer.from('signed Gutenberg plugin package fixture')
const pluginSignature = sign(null, pluginBytes, privateKey).toString('base64')

function writeFixture({
  latestSignature = 'signed updater fixture',
  signatureFile = latestSignature,
  packageUrl = `${releaseBaseUrl}/${pluginName}`,
  packageBytes = pluginBytes,
  extraFile = '',
} = {}) {
  rmSync(candidateDir, { recursive: true, force: true })
  mkdirSync(candidateDir, { recursive: true })
  writeFileSync(configPath, `${JSON.stringify({ version })}\n`)
  writeFileSync(
    pluginTrustPath,
    `pub const OFFICIAL_PLUGIN_KEYS: &[TrustedPluginKey] = &[TrustedPluginKey {\n  key_id: "lnr-plugin-test",\n  public_key_base64: "${publicKeyBytes.toString('base64')}",\n}];\n`,
  )
  writeFileSync(join(candidateDir, installerName), 'signed installer fixture')
  writeFileSync(join(candidateDir, `${installerName}.sig`), `${signatureFile}\n`)
  writeFileSync(join(candidateDir, pluginName), packageBytes)
  writeFileSync(join(candidateDir, 'latest.json'), `${JSON.stringify({
    version,
    notes: 'fixture',
    pub_date: '2026-08-18T00:00:00.000Z',
    platforms: {
      'windows-x86_64': {
        signature: latestSignature,
        url: `${releaseBaseUrl}/${installerName}`,
      },
    },
  }, null, 2)}\n`)
  writeFileSync(join(candidateDir, 'repository.json'), `${JSON.stringify({
    schemaVersion: '0.1',
    generatedAt: 1,
    entries: [{
      manifest: { id: 'gutenberg', version: '0.1.0' },
      packageUrl,
      packageSha256: sha256(packageBytes),
      packageSize: packageBytes.length,
      signature: {
        algorithm: 'ed25519',
        keyId: 'lnr-plugin-test',
        value: pluginSignature,
      },
    }],
  }, null, 2)}\n`)
  if (extraFile) writeFileSync(join(candidateDir, extraFile), 'must be rejected')
}

function check() {
  return verifyReleaseCandidate({ candidateDir, configPath, pluginTrustPath })
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
  const valid = check()
  assertCheck(valid.ok, 'valid unified release candidate should pass', valid)
  assertCheck(valid.assets.length === 5, 'one-plugin candidate should contain exactly five assets', valid)

  writeFixture({ latestSignature: 'manifest signature', signatureFile: 'different signature' })
  const mismatchedUpdaterSignature = check()
  assertCheck(!mismatchedUpdaterSignature.ok, 'mismatched updater signature should fail', mismatchedUpdaterSignature)

  writeFixture({ packageUrl: 'https://github.com/haryqs/lightnovel-reader/releases/download/v0.3.1/gutenberg.zip' })
  const staleTag = check()
  assertCheck(!staleTag.ok, 'stale release tag should fail', staleTag)

  writeFixture({ packageBytes: Buffer.from('tampered plugin package') })
  const tamperedPlugin = check()
  assertCheck(!tamperedPlugin.ok, 'tampered plugin package should fail signature verification', tamperedPlugin)

  writeFixture({ extraFile: 'updater-private.key' })
  const unexpectedSecret = check()
  assertCheck(!unexpectedSecret.ok, 'unexpected files should fail the release allowlist', unexpectedSecret)

  console.log('test-verify-release-candidate: OK')
} finally {
  cleanup()
}
