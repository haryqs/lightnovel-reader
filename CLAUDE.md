# Claude 协作入口

> 本文件与 `AGENTS.md` 保持同义。Claude / Codex / 其他 AI 进入项目时，都按同一套项目记忆和开发纪律工作。

## 阅读顺序

0. `docs/README.md`（文档地图，先看）
1. `docs/dev-memory/PROJECT_MEMORY.md`
2. `docs/dev-memory/DEVELOPMENT_OUTLINE.md`
3. `docs/dev-memory/NEXT_ACTIONS.md`
4. `docs/dev-memory/工程约定与陷阱.md`
5. `docs/resource-library-plan/0_方案总览.md`
6. `docs/resource-library-plan/7_终局架构_多端与插件运行时.md`
7. `docs/resource-library-plan/8_桥接协议_v0.1.md`

## 不要重开争论

- 不换 Electron / Flutter / 纯 Web。
- 不把 Calibre 当底层书库，只作为导入来源。
- 不做盗版源聚合。
- 不绕过付费、登录、DRM。
- 不把 Tauri command 写成业务层。

## 必须维护的项目记忆

- `docs/dev-memory/DEV_LOG.md`：每轮开发追加日志。
- `docs/dev-memory/DECISIONS.md`：记录架构和产品取舍。
- `docs/dev-memory/NEXT_ACTIONS.md`：维护下一步任务队列。
- `docs/resource-library-plan/8_桥接协议_v0.1.md`：协议变更必须同步。

## 项目本地技能

- `.codex/skills/project-memory-maintainer/SKILL.md`：维护项目记忆、开发日志、决策日志。
- `.codex/skills/dev-workflow-runner/SKILL.md`：开工/收工/验证/提交流程。
- `.codex/skills/architecture-guard/SKILL.md`：协议、平台、core 边界纪律。

## 推荐检查命令

```powershell
node scripts/check-arch.mjs
node scripts/check-dev-memory.mjs
node scripts/dev-note.mjs --done "完成内容" --verify "验证结果" --next "下一步"
npm.cmd run build
cargo test --workspace
```

如果依赖未安装或网络不可用，先写清楚阻塞原因，不要声称验证通过。
