import { existsSync, mkdirSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..')
const cargoLockPath = join(repoRoot, 'Cargo.lock')
const outputDir = join(repoRoot, 'src', 'worker', 'reading-core-wasm')
const compiledWasmPath = join(
  repoRoot,
  'target',
  'wasm32-unknown-unknown',
  'release',
  'reading_core.wasm',
)

const fail = (message) => {
  console.error(`build-reading-core-wasm: ${message}`)
  process.exit(1)
}

const capture = (command, args) => {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: 'utf8',
    shell: false,
  })
  if (result.error) fail(`${command} 不可用：${result.error.message}`)
  if (result.status !== 0) {
    process.stderr.write(result.stderr ?? '')
    fail(`${command} ${args.join(' ')} 执行失败`)
  }
  return (result.stdout ?? '').trim()
}

const run = (command, args) => {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    stdio: 'inherit',
    shell: false,
  })
  if (result.error) fail(`${command} 不可用：${result.error.message}`)
  if (result.status !== 0) fail(`${command} ${args.join(' ')} 执行失败`)
}

const cargoLock = readFileSync(cargoLockPath, 'utf8')
const lockedVersion = cargoLock.match(
  /name = "wasm-bindgen"\r?\nversion = "([^"]+)"/,
)?.[1]
if (!lockedVersion) fail('无法从 Cargo.lock 读取 wasm-bindgen 版本')

const installedTargets = capture('rustup', ['target', 'list', '--installed'])
if (!installedTargets.split(/\r?\n/).includes('wasm32-unknown-unknown')) {
  fail('缺少 Rust WASM 目标；请先运行 rustup target add wasm32-unknown-unknown')
}

const cliVersionOutput = capture('wasm-bindgen', ['--version'])
const cliVersion = cliVersionOutput.match(/wasm-bindgen\s+([^\s]+)/)?.[1]
if (cliVersion !== lockedVersion) {
  fail(
    `wasm-bindgen CLI 版本为 ${cliVersion ?? '未知'}，Cargo.lock 需要 ${lockedVersion}；` +
      `请运行 cargo install wasm-bindgen-cli --version ${lockedVersion} --locked --force`,
  )
}

run('cargo', [
  'build',
  '-p',
  'reading-core',
  '--release',
  '--target',
  'wasm32-unknown-unknown',
  '--no-default-features',
  '--features',
  'wasm',
])

if (!existsSync(compiledWasmPath)) {
  fail(`Cargo 构建成功，但未找到 ${compiledWasmPath}`)
}

mkdirSync(outputDir, { recursive: true })
run('wasm-bindgen', [
  compiledWasmPath,
  '--out-dir',
  outputDir,
  '--target',
  'web',
  '--out-name',
  'reading_core',
])

console.log(`build-reading-core-wasm: OK (wasm-bindgen ${lockedVersion})`)
