import { createHash } from 'node:crypto'
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { tmpdir } from 'node:os'
import { spawnSync } from 'node:child_process'

function readOption(name, fallback = '') {
  const idx = process.argv.indexOf(name)
  if (idx >= 0 && process.argv[idx + 1]) return process.argv[idx + 1]
  return fallback
}

function fail(message) {
  console.error(`new-smoke-plugin-repository: ${message}`)
  process.exit(1)
}

const outDir = resolve(readOption('--out-dir', join(tmpdir(), 'lnr-plugin-repository-smoke')))
const baseUrl = readOption('--base-url', 'https://plugins.example.invalid/smoke')
const pluginId = readOption('--plugin-id', 'aozora-smoke-source')

if (!baseUrl.startsWith('https://')) {
  fail('--base-url must be HTTPS because official repository commands reject non-HTTPS package URLs')
}

const packageDir = join(outDir, 'package')
const zipPath = join(outDir, `${pluginId}.zip`)
const repositoryPath = join(outDir, 'repository.json')

rmSync(outDir, { recursive: true, force: true })
mkdirSync(packageDir, { recursive: true })

const manifest = {
  apiVersion: '0.1',
  id: pluginId,
  name: 'Aozora Smoke Source',
  version: '0.1.0',
  description: 'Smoke-test source plugin fixture. It is installed for policy and UI verification only.',
  entry: 'plugin.js',
  domains: ['example.org'],
  permissions: ['http'],
  capabilities: ['browse', 'fetchMetadata'],
  legal: { kind: 'public-domain' },
}

writeFileSync(join(packageDir, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`, 'utf8')
writeFileSync(
  join(packageDir, 'plugin.js'),
  [
    'export async function search() {',
    '  return []',
    '}',
    '',
    'export async function getBook() {',
    '  return null',
    '}',
    '',
    'export async function getChapter() {',
    '  return null',
    '}',
    '',
  ].join('\n'),
  'utf8',
)

const compress = spawnSync(
  'powershell',
  [
    '-NoProfile',
    '-ExecutionPolicy',
    'Bypass',
    '-Command',
    `Compress-Archive -Path '${packageDir.replaceAll("'", "''")}\\*' -DestinationPath '${zipPath.replaceAll("'", "''")}' -Force`,
  ],
  { encoding: 'utf8' },
)
if (compress.status !== 0 || !existsSync(zipPath)) {
  fail(`failed to create zip\n${compress.stderr || compress.stdout}`)
}

const bytes = readFileSync(zipPath)
const packageSha256 = createHash('sha256').update(bytes).digest('hex')
const packageUrl = `${baseUrl.replace(/\/+$/, '')}/${pluginId}.zip`

const repository = {
  schemaVersion: '0.1',
  generatedAt: Date.now(),
  entries: [
    {
      manifest,
      packageUrl,
      packageSha256,
      packageSize: bytes.length,
      sourceUrl: 'https://github.com/haryqs/lightnovel-reader',
    },
  ],
}

writeFileSync(repositoryPath, `${JSON.stringify(repository, null, 2)}\n`, 'utf8')

console.log('new-smoke-plugin-repository: OK')
console.log(`new-smoke-plugin-repository: outDir=${outDir}`)
console.log(`new-smoke-plugin-repository: repository=${repositoryPath}`)
console.log(`new-smoke-plugin-repository: package=${zipPath}`)
console.log(`new-smoke-plugin-repository: packageUrl=${packageUrl}`)
console.log(`new-smoke-plugin-repository: packageSha256=${packageSha256}`)
