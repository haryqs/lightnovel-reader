# AI 协作入口

> 本文件是新 AI / 新线程进入 `lightnovel-reader` 时的第一入口。目标是减少重复解释，让项目记忆留在仓库里，而不是散在聊天记录里。

## 阅读顺序

0. `docs/README.md` —— 文档地图（全部文档分类导航，先看这个）。
1. `docs/dev-memory/PROJECT_MEMORY.md` —— 当前目标、边界、技术纪律、已知风险。
2. `docs/dev-memory/DEVELOPMENT_OUTLINE.md` —— 从 v0.3 到 v1.0 的开发大纲。
3. `docs/dev-memory/NEXT_ACTIONS.md` —— 下一步任务队列。
4. `docs/dev-memory/工程约定与陷阱.md` —— 代码/协作约定与会再踩的工程陷阱。
5. `docs/resource-library-plan/0_方案总览.md` —— 资源书库长期路线。
6. `docs/resource-library-plan/7_终局架构_多端与插件运行时.md` —— 多端与插件运行时终局架构。
7. `docs/resource-library-plan/8_桥接协议_v0.1.md` —— engine/core/平台壳之间的协议边界。

## 开工纪律

- 如果任务涉及项目记忆/文档修订，先读 `.codex/skills/project-memory-maintainer/SKILL.md`。
- 如果任务涉及开工、收工、验证、提交，先读 `.codex/skills/dev-workflow-runner/SKILL.md`。
- 如果任务涉及 core/engine/platform/协议边界，先读 `.codex/skills/architecture-guard/SKILL.md`。
- 先运行 `git status -sb`，确认是否有未提交改动。
- 改代码前读相关模块，不凭记忆改。
- 前端业务代码不得直接 import `@tauri-apps/*`，只能通过 `src/platform/`。
- 业务逻辑优先放进 `crates/reading-core`，Tauri command 只做参数搬运。
- 改协议必须同步三处：`src/platform/protocol.ts`、Rust serde/command、`docs/resource-library-plan/8_桥接协议_v0.1.md`。
- 不引入新依赖，除非收益明确且已经写入开发记录。

## 收工纪律

- 更新 `docs/dev-memory/DEV_LOG.md`：写明做了什么、验证了什么、还欠什么。
- 如有架构/产品取舍，更新 `docs/dev-memory/DECISIONS.md`。
- 如有下一步任务，更新 `docs/dev-memory/NEXT_ACTIONS.md`。
- 至少运行 `node scripts/check-arch.mjs` 和 `node scripts/check-dev-memory.mjs`。
- 可用 `node scripts/dev-note.mjs --done "..." --verify "..." --next "..."` 快速追加开发日志。
- 如果因为依赖、网络、GUI 环境不能验证，必须在最终说明和开发日志里写清楚。

## 当前优先级

1. v0.3 本地书库闭环。
2. 单本 EPUB 导入、封面提取、series 元数据。
3. 实机冒烟测试：开书、翻页、关闭重开恢复进度、高亮重开仍可见。
4. v0.4 标注增强。
5. v0.5 在线元数据与合法入口。
