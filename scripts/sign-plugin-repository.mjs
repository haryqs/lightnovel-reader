import { createHash, createPrivateKey, createPublicKey, sign } from 'node:crypto'
import { basename, dirname, join, resolve } from 'node:path'
import { readFileSync, writeFileSync } from 'node:fs'

function readOption(name, fallback = '') {
  const index = process.argv.indexOf(name)
  return index >= 0 && process.argv[index + 1] ? process.argv[index + 1] : fallback
}

function fail(message) {
  console.error(`sign-plugin-repository: ${message}`)
  process.exit(1)
}

const repositoryPath = resolve(readOption('--repository'))
const privateKeyPath = resolve(readOption('--private-key'))
const keyId = readOption('--key-id').trim()
const expectedPublicKeyBase64 = readOption('--expected-public-key-base64').trim()
const packageDir = resolve(readOption('--package-dir', dirname(repositoryPath)))
const outputPath = resolve(readOption('--out', `${repositoryPath}.signed.json`))

if (!readOption('--repository')) fail('--repository is required')
if (!readOption('--private-key')) fail('--private-key is required')
if (!keyId || keyId.length > 128) fail('--key-id must be 1..128 characters')

let repository
try {
  repository = JSON.parse(readFileSync(repositoryPath, 'utf8'))
} catch (error) {
  fail(`cannot read repository JSON: ${error.message}`)
}
if (!Array.isArray(repository.entries)) fail('repository.entries must be an array')

let privateKey
try {
  privateKey = createPrivateKey(readFileSync(privateKeyPath))
} catch (error) {
  fail(`cannot read PKCS#8 PEM private key: ${error.message}`)
}
if (privateKey.asymmetricKeyType !== 'ed25519') {
  fail(`private key must be Ed25519, got ${privateKey.asymmetricKeyType || 'unknown'}`)
}

const publicKeyDer = createPublicKey(privateKey).export({ type: 'spki', format: 'der' })
const publicKeyBase64 = publicKeyDer.subarray(publicKeyDer.length - 32).toString('base64')
if (expectedPublicKeyBase64 && publicKeyBase64 !== expectedPublicKeyBase64) {
  fail('private key does not match --expected-public-key-base64')
}

for (const entry of repository.entries) {
  let packageName
  try {
    packageName = basename(decodeURIComponent(new URL(entry.packageUrl).pathname))
  } catch {
    fail(`invalid packageUrl for ${entry?.manifest?.id || 'unknown plugin'}`)
  }
  if (!packageName) fail(`packageUrl has no file name for ${entry?.manifest?.id || 'unknown plugin'}`)
  const packagePath = join(packageDir, packageName)
  const bytes = readFileSync(packagePath)
  const sha256 = createHash('sha256').update(bytes).digest('hex')
  if (sha256.toLowerCase() !== String(entry.packageSha256 || '').toLowerCase()) {
    fail(`packageSha256 mismatch for ${entry?.manifest?.id || packageName}`)
  }
  if (entry.packageSize !== undefined && entry.packageSize !== bytes.length) {
    fail(`packageSize mismatch for ${entry?.manifest?.id || packageName}`)
  }
  entry.signature = {
    algorithm: 'ed25519',
    keyId,
    value: sign(null, bytes, privateKey).toString('base64'),
  }
}

writeFileSync(outputPath, `${JSON.stringify(repository, null, 2)}\n`, 'utf8')
console.log('sign-plugin-repository: OK')
console.log(`sign-plugin-repository: entries=${repository.entries.length}`)
console.log(`sign-plugin-repository: output=${outputPath}`)
console.log(`sign-plugin-repository: keyId=${keyId}`)
console.log(`sign-plugin-repository: publicKeyBase64=${publicKeyBase64}`)
