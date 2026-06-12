import { appendFileSync, existsSync } from 'node:fs'

const args = process.argv.slice(2)

const getArg = (name) => {
  const prefix = `--${name}=`
  const exact = `--${name}`
  const idx = args.findIndex(a => a === exact || a.startsWith(prefix))
  if (idx < 0) return ''
  const value = args[idx]
  if (value.startsWith(prefix)) return value.slice(prefix.length)
  return args[idx + 1] || ''
}

const done = getArg('done')
const files = getArg('files')
const verify = getArg('verify')
const blocked = getArg('blocked')
const next = getArg('next')

if (!done) {
  console.error('usage: node scripts/dev-note.mjs --done "完成内容" [--files "..."] [--verify "..."] [--blocked "..."] [--next "..."]')
  process.exit(1)
}

const path = 'docs/dev-memory/DEV_LOG.md'
if (!existsSync(path)) {
  console.error(`dev-note: missing ${path}`)
  process.exit(1)
}

const today = new Date().toISOString().slice(0, 10)
const entry = [
  '',
  `## ${today}：${done}`,
  '',
  '变更：',
  '',
  `- ${done}`,
  '',
  '修改文件：',
  '',
  files ? files.split(',').map(f => `- \`${f.trim()}\``).join('\n') : '- 待补充',
  '',
  '验证：',
  '',
  verify ? verify.split(';').map(v => `- ${v.trim()}`).join('\n') : '- 待补充',
  '',
  '未验证/阻塞：',
  '',
  blocked ? blocked.split(';').map(v => `- ${v.trim()}`).join('\n') : '- 无',
  '',
  '下一步：',
  '',
  next ? next.split(';').map(v => `- ${v.trim()}`).join('\n') : '- 待补充',
  '',
].join('\n')

appendFileSync(path, entry, 'utf8')
console.log(`dev-note: appended to ${path}`)
