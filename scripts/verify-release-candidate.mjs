import { createHash, createPublicKey, verify } from 'node:crypto'
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const scriptPath = fileURLToPath(import.meta.url)
const repoRoot = resolve(dirname(scriptPath), '..')
const githubAssetNamePattern = /^[0-9A-Za-z._-]+$/

function readOption(args, name, fallback) {
  const inline = args.find((arg) => arg.startsWith(`${name}=`))
  if (inline) return inline.slice(name.length + 1)
  const index = args.indexOf(name)
  return index >= 0 && index + 1 < args.length ? args[index + 1] : fallback
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

function readJson(path, label, errors) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch (error) {
    errors.push(`${label} cannot be read as JSON: ${error.message}`)
    return null
  }
}

function decodeBase64Exact(value, expectedLength, label, errors) {
  if (typeof value !== 'string' || !/^[A-Za-z0-9+/]+={0,2}$/.test(value)) {
    errors.push(`${label} must be standard Base64`)
    return null
  }
  const bytes = Buffer.from(value, 'base64')
  if (bytes.length !== expectedLength || bytes.toString('base64') !== value) {
    errors.push(`${label} must decode to exactly ${expectedLength} bytes`)
    return null
  }
  return bytes
}

function parseTrustedPluginKeys(source, errors) {
  const keyringBody = source.match(
    /OFFICIAL_PLUGIN_KEYS[\s\S]*?=\s*&\[([\s\S]*?)\]\s*;/,
  )?.[1] || ''
  const keys = [...keyringBody.matchAll(
    /TrustedPluginKey\s*\{[\s\S]*?key_id:\s*"([^"]+)"\s*,[\s\S]*?public_key_base64:\s*"([^"]+)"\s*,?[\s\S]*?\}/g,
  )]
  const keyring = new Map()
  for (const match of keys) {
    const [, keyId, publicKeyBase64] = match
    if (keyring.has(keyId)) {
      errors.push(`plugin trust contains duplicate keyId: ${keyId}`)
      continue
    }
    const publicKeyBytes = decodeBase64Exact(
      publicKeyBase64,
      32,
      `plugin public key ${keyId}`,
      errors,
    )
    if (!publicKeyBytes) continue
    keyring.set(keyId, createPublicKey({
      key: Buffer.concat([
        Buffer.from('302a300506032b6570032100', 'hex'),
        publicKeyBytes,
      ]),
      format: 'der',
      type: 'spki',
    }))
  }
  if (keyring.size === 0) errors.push('plugin trust must contain at least one valid public key')
  return keyring
}

function assetNameFromReleaseUrl(value, label, releaseBaseUrl, errors) {
  let url
  try {
    url = new URL(value)
  } catch {
    errors.push(`${label} must be a valid URL`)
    return null
  }
  if (url.protocol !== 'https:' || url.username || url.password || url.search || url.hash) {
    errors.push(`${label} must be a credential-free HTTPS URL without query or fragment`)
    return null
  }
  const prefix = `${releaseBaseUrl}/`
  if (!url.toString().startsWith(prefix)) {
    errors.push(`${label} must target ${releaseBaseUrl}`)
    return null
  }
  const encodedName = url.toString().slice(prefix.length)
  let name
  try {
    name = decodeURIComponent(encodedName)
  } catch {
    errors.push(`${label} contains an invalid encoded asset name`)
    return null
  }
  if (!name || name.includes('/') || name.includes('\\') || !githubAssetNamePattern.test(name)) {
    errors.push(`${label} must end in one GitHub-safe asset name`)
    return null
  }
  if (`${prefix}${encodeURIComponent(name)}` !== url.toString()) {
    errors.push(`${label} must use the canonical encoded asset URL`)
    return null
  }
  return name
}

function validateFile(candidateDir, name, label, errors) {
  const path = join(candidateDir, name)
  if (!existsSync(path)) {
    errors.push(`${label} is missing: ${name}`)
    return null
  }
  const info = statSync(path)
  if (!info.isFile()) {
    errors.push(`${label} must be a regular file: ${name}`)
    return null
  }
  if (info.size === 0) {
    errors.push(`${label} must not be empty: ${name}`)
    return null
  }
  return readFileSync(path)
}

export function verifyReleaseCandidate({
  candidateDir,
  configPath,
  pluginTrustPath,
  repositorySlug = 'haryqs/lightnovel-reader',
  tag,
}) {
  const errors = []
  const resolvedCandidateDir = resolve(candidateDir)
  const resolvedConfigPath = resolve(configPath)
  const resolvedPluginTrustPath = resolve(pluginTrustPath)

  if (!existsSync(resolvedCandidateDir) || !statSync(resolvedCandidateDir).isDirectory()) {
    return { ok: false, errors: [`candidate directory is missing: ${resolvedCandidateDir}`], assets: [] }
  }
  if (!/^[0-9A-Za-z_.-]+\/[0-9A-Za-z_.-]+$/.test(repositorySlug)) {
    errors.push('repository slug must be owner/name using GitHub-safe characters')
  }

  const config = readJson(resolvedConfigPath, 'Tauri config', errors)
  const version = config?.version
  if (typeof version !== 'string' || !/^\d+\.\d+\.\d+$/.test(version)) {
    errors.push('Tauri config version must be a three-part SemVer')
  }
  const releaseTag = tag || (version ? `v${version}` : '')
  if (version && releaseTag !== `v${version}`) {
    errors.push(`release tag must be v${version}`)
  }
  const releaseBaseUrl = `https://github.com/${repositorySlug}/releases/download/${releaseTag}`

  const latest = readJson(join(resolvedCandidateDir, 'latest.json'), 'latest.json', errors)
  const repository = readJson(join(resolvedCandidateDir, 'repository.json'), 'repository.json', errors)
  const expectedFiles = new Set(['latest.json', 'repository.json'])

  if (latest?.version !== version) {
    errors.push(`latest.json version must match Tauri version ${version || '<invalid>'}`)
  }
  const platformNames = latest?.platforms && typeof latest.platforms === 'object'
    ? Object.keys(latest.platforms)
    : []
  if (platformNames.length !== 1 || platformNames[0] !== 'windows-x86_64') {
    errors.push('latest.json must contain exactly the windows-x86_64 updater platform')
  }
  const windowsUpdater = latest?.platforms?.['windows-x86_64']
  const installerName = assetNameFromReleaseUrl(
    windowsUpdater?.url,
    'latest.json updater URL',
    releaseBaseUrl,
    errors,
  )
  if (installerName) {
    const expectedInstallerName = `LightNovel.Reader_${version}_x64-setup.exe`
    if (installerName !== expectedInstallerName) {
      errors.push(`updater installer asset must be named ${expectedInstallerName}`)
    }
    expectedFiles.add(installerName)
    expectedFiles.add(`${installerName}.sig`)
    validateFile(resolvedCandidateDir, installerName, 'updater installer', errors)
    const signatureBytes = validateFile(
      resolvedCandidateDir,
      `${installerName}.sig`,
      'updater signature',
      errors,
    )
    if (signatureBytes) {
      const signature = signatureBytes.toString('utf8').trim()
      if (!signature || windowsUpdater?.signature !== signature) {
        errors.push('latest.json updater signature must exactly match the .sig file contents')
      }
    }
  }

  if (repository?.schemaVersion !== '0.1') {
    errors.push('repository.json schemaVersion must be 0.1')
  }
  if (!Array.isArray(repository?.entries) || repository.entries.length === 0) {
    errors.push('repository.json must contain at least one signed plugin entry')
  }

  let trustSource = ''
  try {
    trustSource = readFileSync(resolvedPluginTrustPath, 'utf8')
  } catch (error) {
    errors.push(`plugin trust cannot be read: ${error.message}`)
  }
  const trustedKeys = parseTrustedPluginKeys(trustSource, errors)
  const seenPluginIds = new Set()
  for (const entry of Array.isArray(repository?.entries) ? repository.entries : []) {
    const pluginId = entry?.manifest?.id
    if (typeof pluginId !== 'string' || !pluginId) {
      errors.push('every repository entry must have a manifest id')
      continue
    }
    if (seenPluginIds.has(pluginId)) errors.push(`repository contains duplicate plugin id: ${pluginId}`)
    seenPluginIds.add(pluginId)

    const packageName = assetNameFromReleaseUrl(
      entry.packageUrl,
      `plugin ${pluginId} package URL`,
      releaseBaseUrl,
      errors,
    )
    if (!packageName || !packageName.toLowerCase().endsWith('.zip')) {
      if (packageName) errors.push(`plugin ${pluginId} package asset must be a .zip file`)
      continue
    }
    expectedFiles.add(packageName)
    const packageBytes = validateFile(resolvedCandidateDir, packageName, `plugin ${pluginId} package`, errors)
    if (!packageBytes) continue
    if (sha256(packageBytes) !== String(entry.packageSha256 || '').toLowerCase()) {
      errors.push(`plugin ${pluginId} package SHA-256 mismatch`)
    }
    if (entry.packageSize !== packageBytes.length) {
      errors.push(`plugin ${pluginId} package size mismatch`)
    }

    const signature = entry.signature
    if (signature?.algorithm !== 'ed25519') {
      errors.push(`plugin ${pluginId} signature algorithm must be ed25519`)
      continue
    }
    const publicKey = trustedKeys.get(signature?.keyId)
    if (!publicKey) {
      errors.push(`plugin ${pluginId} signature keyId is not compiled into plugin trust`)
      continue
    }
    const signatureBytes = decodeBase64Exact(
      signature?.value,
      64,
      `plugin ${pluginId} signature`,
      errors,
    )
    if (signatureBytes && !verify(null, packageBytes, publicKey, signatureBytes)) {
      errors.push(`plugin ${pluginId} signature verification failed`)
    }
  }

  const actualEntries = readdirSync(resolvedCandidateDir, { withFileTypes: true })
  for (const entry of actualEntries) {
    if (!entry.isFile()) errors.push(`release candidate must not contain non-file entries: ${entry.name}`)
  }
  const actualFiles = actualEntries.filter((entry) => entry.isFile()).map((entry) => entry.name).sort()
  for (const name of expectedFiles) {
    if (!actualFiles.includes(name)) errors.push(`expected release asset is missing: ${name}`)
  }
  for (const name of actualFiles) {
    if (!expectedFiles.has(name)) errors.push(`unexpected release asset is not allowed: ${name}`)
  }

  const assets = actualFiles
    .filter((name) => expectedFiles.has(name))
    .map((name) => {
      const bytes = readFileSync(join(resolvedCandidateDir, name))
      return { name, size: bytes.length, sha256: sha256(bytes) }
    })

  return {
    ok: errors.length === 0,
    errors,
    version: version || null,
    tag: releaseTag || null,
    assets,
  }
}

function main() {
  const args = process.argv.slice(2)
  const candidateDir = readOption(args, '--dir', '')
  if (!candidateDir) {
    console.error('verify-release-candidate: --dir is required')
    process.exitCode = 1
    return
  }
  const result = verifyReleaseCandidate({
    candidateDir,
    configPath: readOption(args, '--config', resolve(repoRoot, 'src-tauri', 'tauri.conf.json')),
    pluginTrustPath: readOption(
      args,
      '--plugin-trust',
      resolve(repoRoot, 'src-tauri', 'src', 'plugin_trust.rs'),
    ),
    repositorySlug: readOption(args, '--repository-slug', 'haryqs/lightnovel-reader'),
    tag: readOption(args, '--tag', ''),
  })
  if (!result.ok) {
    console.error('verify-release-candidate: BLOCKED')
    for (const error of result.errors) console.error(`- ${error}`)
    process.exitCode = 1
    return
  }
  console.log(`verify-release-candidate: OK(version=${result.version}, tag=${result.tag}, assets=${result.assets.length})`)
  for (const asset of result.assets) {
    console.log(`${asset.sha256}  ${asset.size}  ${asset.name}`)
  }
}

if (resolve(process.argv[1] || '') === resolve(scriptPath)) main()
