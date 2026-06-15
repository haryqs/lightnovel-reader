import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const requiredFiles = [
  'AGENTS.md',
  'CLAUDE.md',
  'docs/dev-memory/PROJECT_MEMORY.md',
  'docs/dev-memory/DEVELOPMENT_OUTLINE.md',
  'docs/dev-memory/DECISIONS.md',
  'docs/dev-memory/DEV_LOG.md',
  'docs/dev-memory/NEXT_ACTIONS.md',
  'docs/dev-memory/SESSION_TEMPLATE.md',
  'docs/dev-memory/TOOLING_BACKLOG.md',
  'docs/dev-memory/工程约定与陷阱.md',
  'docs/README.md',
  'docs/resource-library-plan/8_桥接协议_v0.1.md',
  '.codex/skills/project-memory-maintainer/SKILL.md',
  '.codex/skills/dev-workflow-runner/SKILL.md',
  '.codex/skills/architecture-guard/SKILL.md',
]

const requiredText = new Map([
  ['AGENTS.md', ['阅读顺序', '开工纪律', '收工纪律']],
  ['CLAUDE.md', ['阅读顺序', '必须维护的项目记忆']],
  ['docs/dev-memory/PROJECT_MEMORY.md', ['项目定位', '不可变约束', '架构纪律']],
  ['docs/dev-memory/DEVELOPMENT_OUTLINE.md', ['当前基线', '近期冲刺', 'v0.3']],
  ['docs/dev-memory/DEV_LOG.md', ['开发日志']],
  ['docs/dev-memory/NEXT_ACTIONS.md', ['P0', 'P1']],
  ['.codex/skills/project-memory-maintainer/SKILL.md', ['Project Memory Maintainer']],
  ['.codex/skills/dev-workflow-runner/SKILL.md', ['Dev Workflow Runner']],
  ['.codex/skills/architecture-guard/SKILL.md', ['Architecture Guard']],
])

let failed = false

for (const file of requiredFiles) {
  const path = resolve(file)
  if (!existsSync(path)) {
    console.error(`check-dev-memory: missing ${file}`)
    failed = true
    continue
  }

  const text = readFileSync(path, 'utf8')
  const needles = requiredText.get(file) || []
  for (const needle of needles) {
    if (!text.includes(needle)) {
      console.error(`check-dev-memory: ${file} missing text "${needle}"`)
      failed = true
    }
  }
}

if (failed) {
  process.exit(1)
}

console.log('check-dev-memory: OK(项目记忆文件完整)')
