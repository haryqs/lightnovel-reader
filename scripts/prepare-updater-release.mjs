import {
  constants,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from 'node:fs'
import { basename, join, resolve } from 'node:path'

function readOption(name, fallback = '') {
  const index = process.argv.indexOf(name)
  return index >= 0 && process.argv[index + 1] ? process.argv[index + 1] : fallback
}

function hasFlag(name) {
  return process.argv.includes(name)
}

function fail(message) {
  console.error(`prepare-updater-release: ${message}`)
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

const bundleDir = resolve(readOption('--bundle-dir', 'target/release/bundle'))
const configPath = resolve(readOption('--config', 'src-tauri/tauri.conf.json'))
const outDirOption = readOption('--out-dir')
const baseUrlOption = readOption('--base-url')
const versionOption = readOption('--version').trim()
const notes = readOption('--notes', '')
const pubDateOption = readOption('--pub-date')
const force = hasFlag('--force')

if (!outDirOption) fail('--out-dir is required')
if (!baseUrlOption) fail('--base-url is required')

let configuredVersion
try {
  configuredVersion = JSON.parse(readFileSync(configPath, 'utf8')).version
} catch (error) {
  fail(`cannot read Tauri config version: ${error.message}`)
}
if (typeof configuredVersion !== 'string') fail(`Tauri config has no string version: ${configPath}`)
const version = versionOption || configuredVersion
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
  fail('--version must be a valid SemVer without a leading v')
}
if (version !== configuredVersion) {
  fail(`--version ${version} does not match Tauri config version ${configuredVersion}`)
}

const baseUrl = parseHttpsUrl(baseUrlOption, '--base-url')
const pubDate = pubDateOption ? new Date(pubDateOption) : new Date()
if (Number.isNaN(pubDate.getTime())) fail('--pub-date must be a valid RFC 3339 date')

const nsisDir = join(bundleDir, 'nsis')
if (!existsSync(nsisDir)) fail(`NSIS bundle directory not found: ${nsisDir}`)
const installers = readdirSync(nsisDir)
  .filter((name) => name.toLowerCase().endsWith('.exe'))
  .sort()
if (installers.length !== 1) {
  fail(`expected exactly one NSIS .exe in ${nsisDir}, found ${installers.length}`)
}

const installerName = installers[0]
const installerPath = join(nsisDir, installerName)
const signaturePath = `${installerPath}.sig`
if (!existsSync(signaturePath)) fail(`NSIS updater signature not found: ${signaturePath}`)
const signature = readFileSync(signaturePath, 'utf8').trim()
if (!signature) fail(`NSIS updater signature is empty: ${signaturePath}`)

const outDir = resolve(outDirOption)
const outputInstaller = join(outDir, installerName)
const outputSignature = join(outDir, basename(signaturePath))
const latestJsonPath = join(outDir, 'latest.json')
const outputPaths = [outputInstaller, outputSignature, latestJsonPath]
if (!force && outputPaths.some((path) => existsSync(path))) {
  fail(`updater release output already exists in ${outDir}; inspect it or pass --force`)
}

const installerUrl = new URL(
  encodeURIComponent(installerName),
  `${baseUrl.toString().replace(/\/+$/, '')}/`,
)
const latest = {
  version,
  notes,
  pub_date: pubDate.toISOString(),
  platforms: {
    'windows-x86_64': {
      signature,
      url: installerUrl.toString(),
    },
  },
}

mkdirSync(outDir, { recursive: true })
copyFileSync(installerPath, outputInstaller, force ? 0 : constants.COPYFILE_EXCL)
copyFileSync(signaturePath, outputSignature, force ? 0 : constants.COPYFILE_EXCL)
writeFileSync(latestJsonPath, `${JSON.stringify(latest, null, 2)}\n`, {
  encoding: 'utf8',
  flag: force ? 'w' : 'wx',
})

console.log('prepare-updater-release: OK')
console.log(`prepare-updater-release: version=${version}`)
console.log(`prepare-updater-release: installer=${outputInstaller}`)
console.log(`prepare-updater-release: signature=${outputSignature}`)
console.log(`prepare-updater-release: latestJson=${latestJsonPath}`)
console.log(`prepare-updater-release: url=${installerUrl}`)
