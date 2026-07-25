import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const scriptPath = fileURLToPath(import.meta.url)
const repoRoot = resolve(dirname(scriptPath), '..')

function readOption(args, name, fallback) {
  const inline = args.find((arg) => arg.startsWith(`${name}=`))
  if (inline) return inline.slice(name.length + 1)
  const index = args.indexOf(name)
  return index >= 0 && index + 1 < args.length ? args[index + 1] : fallback
}

function decodePublicKey(value) {
  if (!/^[A-Za-z0-9+/]{43}=$/.test(value)) return null
  const bytes = Buffer.from(value, 'base64')
  return bytes.length === 32 ? bytes : null
}

export function checkReleaseTrust({ pluginTrustPath, tauriConfigPath }) {
  const errors = []
  const trustSource = readFileSync(pluginTrustPath, 'utf8')
  const tauriConfig = JSON.parse(readFileSync(tauriConfigPath, 'utf8'))

  if (!/REQUIRE_OFFICIAL_PLUGIN_SIGNATURES\s*:\s*bool\s*=\s*true\s*;/.test(trustSource)) {
    errors.push('REQUIRE_OFFICIAL_PLUGIN_SIGNATURES must be true for release packaging')
  }

  const keyringBody = trustSource.match(
    /OFFICIAL_PLUGIN_KEYS[\s\S]*?=\s*&\[([\s\S]*?)\]\s*;/,
  )?.[1] || ''
  const keys = [...keyringBody.matchAll(
    /TrustedPluginKey\s*\{[\s\S]*?key_id:\s*"([^"]+)"\s*,[\s\S]*?public_key_base64:\s*"([^"]+)"\s*,?[\s\S]*?\}/g,
  )].map((match) => ({ keyId: match[1], publicKeyBase64: match[2] }))

  if (keys.length === 0) {
    errors.push('OFFICIAL_PLUGIN_KEYS must contain at least one release public key')
  }
  const seenKeyIds = new Set()
  for (const key of keys) {
    if (!key.keyId.trim() || [...key.keyId].length > 128) {
      errors.push('plugin release keyId must be 1..128 characters')
    } else if (seenKeyIds.has(key.keyId)) {
      errors.push(`plugin release keyId is duplicated: ${key.keyId}`)
    }
    seenKeyIds.add(key.keyId)
    if (!decodePublicKey(key.publicKeyBase64)) {
      errors.push(`plugin release public key must be standard Base64 for 32 bytes: ${key.keyId || '<empty>'}`)
    }
  }

  const updaterPubkey = tauriConfig?.plugins?.updater?.pubkey
  if (typeof updaterPubkey !== 'string' || !updaterPubkey.trim()) {
    errors.push('tauri updater pubkey must be provisioned for release packaging')
  }
  const createUpdaterArtifacts = tauriConfig?.bundle?.createUpdaterArtifacts
  if (createUpdaterArtifacts !== true) {
    errors.push('tauri bundle.createUpdaterArtifacts must be true for release packaging')
  }

  return {
    ok: errors.length === 0,
    errors,
    pluginKeyIds: keys.map((key) => key.keyId),
    updaterPubkeyConfigured: typeof updaterPubkey === 'string' && updaterPubkey.trim().length > 0,
    updaterArtifactsEnabled: createUpdaterArtifacts === true,
  }
}

function main() {
  const args = process.argv.slice(2)
  const pluginTrustPath = resolve(readOption(
    args,
    '--plugin-trust',
    resolve(repoRoot, 'src-tauri', 'src', 'plugin_trust.rs'),
  ))
  const tauriConfigPath = resolve(readOption(
    args,
    '--tauri-config',
    resolve(repoRoot, 'src-tauri', 'tauri.conf.json'),
  ))
  const result = checkReleaseTrust({ pluginTrustPath, tauriConfigPath })
  if (!result.ok) {
    console.error('check-release-trust: BLOCKED')
    for (const error of result.errors) console.error(`- ${error}`)
    process.exitCode = 1
    return
  }
  console.log(
    `check-release-trust: OK(pluginKeys=${result.pluginKeyIds.join(',')}, updaterPubkey=configured, updaterArtifacts=enabled)`,
  )
}

if (resolve(process.argv[1] || '') === resolve(scriptPath)) main()
