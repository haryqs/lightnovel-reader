// 架构纪律检查(方案文档 7 的纪律 1):
// @tauri-apps/* 只允许出现在 src/platform/ 内,引擎代码一律经 platform 适配层。
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { dirname, join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'

const srcDir = join(dirname(fileURLToPath(import.meta.url)), '..', 'src')
const offenders = []

const walk = (dir) => {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name)
    if (statSync(p).isDirectory()) {
      walk(p)
      continue
    }
    if (!/\.(ts|tsx|js|mjs)$/.test(name)) continue
    const rel = relative(srcDir, p).replace(/\\/g, '/')
    if (rel.startsWith('platform/')) continue
    if (readFileSync(p, 'utf8').includes('@tauri-apps')) offenders.push(rel)
  }
}

walk(srcDir)

if (offenders.length > 0) {
  console.error('架构纪律违规:以下文件绕过 platform 适配层直接使用 @tauri-apps:')
  for (const f of offenders) console.error(`  src/${f}`)
  process.exit(1)
}
console.log('check-arch: OK(@tauri-apps 仅出现在 src/platform/ 内)')
