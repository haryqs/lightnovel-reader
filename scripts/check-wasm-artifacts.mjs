import { readFileSync, statSync } from 'node:fs'
import { dirname, join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..')
const outputDir = join(repoRoot, 'src', 'worker', 'reading-core-wasm')
const requiredFiles = [
  'reading_core.js',
  'reading_core.d.ts',
  'reading_core_bg.wasm',
  'reading_core_bg.wasm.d.ts',
]

const fail = (message) => {
  console.error(`check-wasm-artifacts: ${message}`)
  console.error('请运行 npm run build:wasm 重新生成 reading-core 浏览器产物。')
  process.exit(1)
}

for (const name of requiredFiles) {
  const path = join(outputDir, name)
  let size = 0
  try {
    size = statSync(path).size
  } catch {
    fail(`缺少 ${relative(repoRoot, path)}`)
  }
  if (size === 0) fail(`${relative(repoRoot, path)} 是空文件`)
}

const declarations = readFileSync(join(outputDir, 'reading_core.d.ts'), 'utf8')
for (const exportName of ['parse_epub_metadata', 'get_chapter_html', 'paginate']) {
  if (!declarations.includes(`export function ${exportName}`)) {
    fail(`reading_core.d.ts 缺少 ${exportName} 导出`)
  }
}

const wasmHeader = readFileSync(join(outputDir, 'reading_core_bg.wasm')).subarray(0, 4)
if (!wasmHeader.equals(Buffer.from([0x00, 0x61, 0x73, 0x6d]))) {
  fail('reading_core_bg.wasm 不是有效的 WebAssembly 二进制文件')
}

console.log('check-wasm-artifacts: OK')
