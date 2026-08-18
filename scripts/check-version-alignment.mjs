import { existsSync, readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const scriptPath = fileURLToPath(import.meta.url)
const defaultRepoRoot = resolve(dirname(scriptPath), '..')
const cargoManifestPaths = [
  'src-tauri/Cargo.toml',
  'crates/reading-core/Cargo.toml',
  'crates/sync-server/Cargo.toml',
]

function readOption(args, name, fallback) {
  const inline = args.find((arg) => arg.startsWith(`${name}=`))
  if (inline) return inline.slice(name.length + 1)
  const index = args.indexOf(name)
  return index >= 0 && index + 1 < args.length ? args[index + 1] : fallback
}

function readJson(path, label, errors) {
  if (!existsSync(path)) {
    errors.push(`${label} is missing`)
    return null
  }
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch (error) {
    errors.push(`${label} cannot be parsed: ${error.message}`)
    return null
  }
}

export function checkVersionAlignment(repoRoot) {
  const errors = []
  const tauriConfig = readJson(resolve(repoRoot, 'src-tauri/tauri.conf.json'), 'src-tauri/tauri.conf.json', errors)
  const expectedVersion = tauriConfig?.version
  if (typeof expectedVersion !== 'string' || !/^\d+\.\d+\.\d+$/.test(expectedVersion)) {
    errors.push('Tauri product version must be a three-part SemVer')
  }

  const packageJson = readJson(resolve(repoRoot, 'package.json'), 'package.json', errors)
  if (expectedVersion && packageJson?.version !== expectedVersion) {
    errors.push(`package.json version must match Tauri product version ${expectedVersion}`)
  }

  const packageLock = readJson(resolve(repoRoot, 'package-lock.json'), 'package-lock.json', errors)
  if (expectedVersion && packageLock?.version !== expectedVersion) {
    errors.push(`package-lock.json version must match Tauri product version ${expectedVersion}`)
  }
  if (expectedVersion && packageLock?.packages?.['']?.version !== expectedVersion) {
    errors.push(`package-lock.json root package version must match Tauri product version ${expectedVersion}`)
  }

  for (const relativePath of cargoManifestPaths) {
    const manifestPath = resolve(repoRoot, relativePath)
    if (!existsSync(manifestPath)) {
      errors.push(`${relativePath} is missing`)
      continue
    }
    const manifest = readFileSync(manifestPath, 'utf8')
    const version = manifest.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1]
    if (expectedVersion && version !== expectedVersion) {
      errors.push(`${relativePath} version must match Tauri product version ${expectedVersion}`)
    }
  }

  return { ok: errors.length === 0, errors, version: expectedVersion || null }
}

function main() {
  const repoRoot = resolve(readOption(process.argv.slice(2), '--root', defaultRepoRoot))
  const result = checkVersionAlignment(repoRoot)
  if (!result.ok) {
    console.error('check-version-alignment: BLOCKED')
    for (const error of result.errors) console.error(`- ${error}`)
    process.exitCode = 1
    return
  }
  console.log(`check-version-alignment: OK(version=${result.version}, packages=${cargoManifestPaths.length + 1})`)
}

if (resolve(process.argv[1] || '') === resolve(scriptPath)) main()
