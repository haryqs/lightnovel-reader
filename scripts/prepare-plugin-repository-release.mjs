import { createHash } from 'node:crypto'
import {
  constants,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from 'node:fs'
import { basename, join, resolve } from 'node:path'
import { spawnSync } from 'node:child_process'

function readOption(name, fallback = '') {
  const index = process.argv.indexOf(name)
  return index >= 0 && process.argv[index + 1] ? process.argv[index + 1] : fallback
}

function hasFlag(name) {
  return process.argv.includes(name)
}

function fail(message) {
  console.error(`prepare-plugin-repository-release: ${message}`)
  process.exit(1)
}

function parseHttpsUrl(value, label) {
  let url
  try {
    url = new URL(value)
  } catch {
    fail(`${label} must be a valid HTTPS URL`)
  }
  if (url.protocol !== 'https:' || url.username || url.password) {
    fail(`${label} must be an HTTPS URL without credentials`)
  }
  return url
}

function runTar(args, label) {
  const result = spawnSync('tar', args, {
    encoding: 'utf8',
    windowsHide: true,
  })
  if (result.status !== 0) {
    fail(`${label} failed: ${(result.stderr || result.stdout || 'unknown tar error').trim()}`)
  }
  return result.stdout
}

const packageOption = readOption('--package')
const baseUrlOption = readOption('--base-url')
const outDirOption = readOption('--out-dir')
const sourceUrlOption = readOption('--source-url')
const generatedAtOption = readOption('--generated-at')
const force = hasFlag('--force')

if (!packageOption) fail('--package is required')
if (!baseUrlOption) fail('--base-url is required')
if (!outDirOption) fail('--out-dir is required')

const packagePath = resolve(packageOption)
const packageName = basename(packagePath)
const outDir = resolve(outDirOption)
const outputPackagePath = join(outDir, packageName)
const repositoryPath = join(outDir, 'repository.unsigned.json')

if (!existsSync(packagePath)) fail(`package not found: ${packagePath}`)
if (!packageName.toLowerCase().endsWith('.zip')) fail('--package must point to a .zip file')

const baseUrl = parseHttpsUrl(baseUrlOption, '--base-url')
const sourceUrl = sourceUrlOption ? parseHttpsUrl(sourceUrlOption, '--source-url').toString() : undefined
const generatedAt = generatedAtOption ? Number(generatedAtOption) : Date.now()
if (!Number.isSafeInteger(generatedAt) || generatedAt < 0) {
  fail('--generated-at must be a non-negative safe integer')
}

const archiveEntries = runTar(['-tf', packagePath], 'listing plugin package')
  .split(/\r?\n/)
  .map((entry) => entry.replaceAll('\\', '/').replace(/^\.\/+/, '').replace(/\/+$/, ''))
  .filter(Boolean)
const manifestPaths = archiveEntries.filter(
  (entry) => entry === 'manifest.json' || entry.endsWith('/manifest.json'),
)
if (manifestPaths.length !== 1) {
  fail(`plugin package must contain exactly one manifest.json, found ${manifestPaths.length}`)
}

let manifest
try {
  manifest = JSON.parse(runTar(['-xOf', packagePath, manifestPaths[0]], 'reading plugin manifest'))
} catch (error) {
  fail(`plugin manifest must be valid JSON: ${error.message}`)
}
delete manifest.$schema

if (typeof manifest.id !== 'string' || !manifest.id.trim()) {
  fail('plugin manifest id is required')
}
if (typeof manifest.version !== 'string' || !manifest.version.trim()) {
  fail(`plugin ${manifest.id} manifest version is required`)
}

const bytes = readFileSync(packagePath)
if (bytes.length === 0) fail('plugin package must not be empty')
const packageSha256 = createHash('sha256').update(bytes).digest('hex')
const packageUrl = new URL(encodeURIComponent(packageName), `${baseUrl.toString().replace(/\/+$/, '')}/`)

const entry = {
  manifest,
  packageUrl: packageUrl.toString(),
  packageSha256,
  packageSize: bytes.length,
}
if (sourceUrl) entry.sourceUrl = sourceUrl

const repository = {
  schemaVersion: '0.1',
  generatedAt,
  entries: [entry],
}

mkdirSync(outDir, { recursive: true })
if (!force && (existsSync(outputPackagePath) || existsSync(repositoryPath))) {
  fail(`release output already exists in ${outDir}; inspect it or pass --force`)
}

copyFileSync(packagePath, outputPackagePath, force ? 0 : constants.COPYFILE_EXCL)
writeFileSync(repositoryPath, `${JSON.stringify(repository, null, 2)}\n`, {
  encoding: 'utf8',
  flag: force ? 'w' : 'wx',
})

console.log('prepare-plugin-repository-release: OK')
console.log(`prepare-plugin-repository-release: plugin=${manifest.id}@${manifest.version}`)
console.log(`prepare-plugin-repository-release: package=${outputPackagePath}`)
console.log(`prepare-plugin-repository-release: repository=${repositoryPath}`)
console.log(`prepare-plugin-repository-release: packageUrl=${packageUrl}`)
console.log(`prepare-plugin-repository-release: packageSha256=${packageSha256}`)
