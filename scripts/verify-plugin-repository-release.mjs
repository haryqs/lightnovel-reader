import { createHash, createPublicKey, verify } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import { basename, dirname, join, resolve } from 'node:path'

function readOption(name, fallback = '') {
  const index = process.argv.indexOf(name)
  return index >= 0 && process.argv[index + 1] ? process.argv[index + 1] : fallback
}

function fail(message) {
  console.error(`verify-plugin-repository-release: ${message}`)
  process.exit(1)
}

function decodeBase64Exact(value, expectedLength, label) {
  if (typeof value !== 'string' || !/^[A-Za-z0-9+/]+={0,2}$/.test(value)) {
    fail(`${label} must be standard Base64`)
  }
  const bytes = Buffer.from(value, 'base64')
  if (bytes.length !== expectedLength || bytes.toString('base64') !== value) {
    fail(`${label} must decode to exactly ${expectedLength} bytes`)
  }
  return bytes
}

const repositoryOption = readOption('--repository')
const packageDirOption = readOption('--package-dir')
const publicKeyBase64 = readOption('--public-key-base64').trim()
const expectedKeyId = readOption('--key-id').trim()

if (!repositoryOption) fail('--repository is required')
if (!publicKeyBase64) fail('--public-key-base64 is required')
if (!expectedKeyId) fail('--key-id is required')

const repositoryPath = resolve(repositoryOption)
const packageDir = resolve(packageDirOption || dirname(repositoryPath))
const publicKeyBytes = decodeBase64Exact(publicKeyBase64, 32, 'public key')
const publicKey = createPublicKey({
  key: Buffer.concat([
    Buffer.from('302a300506032b6570032100', 'hex'),
    publicKeyBytes,
  ]),
  format: 'der',
  type: 'spki',
})

let repository
try {
  repository = JSON.parse(readFileSync(repositoryPath, 'utf8'))
} catch (error) {
  fail(`cannot read repository JSON: ${error.message}`)
}
if (repository.schemaVersion !== '0.1') fail('repository schemaVersion must be 0.1')
if (!Array.isArray(repository.entries) || repository.entries.length === 0) {
  fail('repository.entries must contain at least one signed plugin')
}

const seenIds = new Set()
for (const entry of repository.entries) {
  const pluginId = entry?.manifest?.id
  if (typeof pluginId !== 'string' || !pluginId) fail('every repository entry must have a manifest id')
  if (seenIds.has(pluginId)) fail(`repository contains duplicate plugin id: ${pluginId}`)
  seenIds.add(pluginId)

  let packageName
  try {
    packageName = basename(decodeURIComponent(new URL(entry.packageUrl).pathname))
  } catch {
    fail(`plugin ${pluginId} has an invalid packageUrl`)
  }
  if (!packageName) fail(`plugin ${pluginId} packageUrl has no file name`)
  const packagePath = join(packageDir, packageName)
  if (!existsSync(packagePath)) fail(`plugin ${pluginId} package is missing: ${packagePath}`)

  const packageBytes = readFileSync(packagePath)
  const actualSha256 = createHash('sha256').update(packageBytes).digest('hex')
  if (actualSha256 !== String(entry.packageSha256 || '').toLowerCase()) {
    fail(`plugin ${pluginId} package SHA-256 mismatch`)
  }
  if (entry.packageSize !== packageBytes.length) {
    fail(`plugin ${pluginId} package size mismatch`)
  }

  const signature = entry.signature
  if (signature?.algorithm !== 'ed25519') fail(`plugin ${pluginId} signature algorithm must be ed25519`)
  if (signature?.keyId !== expectedKeyId) {
    fail(`plugin ${pluginId} signature keyId must be ${expectedKeyId}`)
  }
  const signatureBytes = decodeBase64Exact(signature?.value, 64, `plugin ${pluginId} signature`)
  if (!verify(null, packageBytes, publicKey, signatureBytes)) {
    fail(`plugin ${pluginId} signature verification failed`)
  }
}

console.log('verify-plugin-repository-release: OK')
console.log(`verify-plugin-repository-release: entries=${repository.entries.length}`)
console.log(`verify-plugin-repository-release: keyId=${expectedKeyId}`)
