import { createHash } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const scriptPath = fileURLToPath(import.meta.url)
const defaultRepoRoot = resolve(dirname(scriptPath), '..')
const expectedLicense = 'AGPL-3.0-only'
const canonicalLicenseSha256 = 'd8a6cc31abc16b6748c7a21f21611f5a1ec33f67d22ca23d7da1c19b95496bee'
const cargoManifestPaths = [
  'src-tauri/Cargo.toml',
  'crates/reading-core/Cargo.toml',
  'crates/sync-server/Cargo.toml',
]

function sha256(content) {
  return createHash('sha256').update(content).digest('hex')
}

function readOption(args, name, fallback) {
  const inline = args.find((arg) => arg.startsWith(`${name}=`))
  if (inline) return inline.slice(name.length + 1)
  const index = args.indexOf(name)
  return index >= 0 && index + 1 < args.length ? args[index + 1] : fallback
}

export function checkOpenSourceLicense(repoRoot) {
  const errors = []
  const licensePath = resolve(repoRoot, 'LICENSE')
  if (!existsSync(licensePath)) {
    errors.push('root LICENSE file is missing')
  } else {
    const actualHash = sha256(readFileSync(licensePath, 'utf8').replaceAll('\r\n', '\n'))
    if (actualHash !== canonicalLicenseSha256) {
      errors.push('root LICENSE must be the unmodified canonical AGPL-3.0-only text')
    }
  }

  const packagePath = resolve(repoRoot, 'package.json')
  if (!existsSync(packagePath)) {
    errors.push('package.json is missing')
  } else {
    try {
      const packageJson = JSON.parse(readFileSync(packagePath, 'utf8'))
      if (packageJson.license !== expectedLicense) {
        errors.push(`package.json license must be ${expectedLicense}`)
      }
    } catch (error) {
      errors.push(`package.json cannot be parsed: ${error.message}`)
    }
  }

  const packageLockPath = resolve(repoRoot, 'package-lock.json')
  if (!existsSync(packageLockPath)) {
    errors.push('package-lock.json is missing')
  } else {
    try {
      const packageLock = JSON.parse(readFileSync(packageLockPath, 'utf8'))
      if (packageLock?.packages?.['']?.license !== expectedLicense) {
        errors.push(`package-lock.json root license must be ${expectedLicense}`)
      }
    } catch (error) {
      errors.push(`package-lock.json cannot be parsed: ${error.message}`)
    }
  }

  for (const relativePath of cargoManifestPaths) {
    const manifestPath = resolve(repoRoot, relativePath)
    if (!existsSync(manifestPath)) {
      errors.push(`${relativePath} is missing`)
      continue
    }
    const source = readFileSync(manifestPath, 'utf8')
    if (!new RegExp(`^license\\s*=\\s*"${expectedLicense}"\\s*$`, 'm').test(source)) {
      errors.push(`${relativePath} license must be ${expectedLicense}`)
    }
  }

  return { ok: errors.length === 0, errors }
}

function main() {
  const repoRoot = resolve(readOption(process.argv.slice(2), '--root', defaultRepoRoot))
  const result = checkOpenSourceLicense(repoRoot)
  if (!result.ok) {
    console.error('check-open-source-license: BLOCKED')
    for (const error of result.errors) console.error(`- ${error}`)
    process.exitCode = 1
    return
  }
  console.log(`check-open-source-license: OK(license=${expectedLicense}, packages=${cargoManifestPaths.length + 1})`)
}

if (resolve(process.argv[1] || '') === resolve(scriptPath)) main()
