# 开发日志

> 每轮开发结束追加。只写会影响未来接手的信息。

## 2026-06-12：建立项目记忆与开发流程

变更：

- 新增根目录 `AGENTS.md` / `CLAUDE.md` 作为 AI 协作入口。
- 新增 `docs/dev-memory/` 项目记忆目录。
- 新增 `scripts/check-dev-memory.mjs` 检查关键记忆文档。
- `package.json` 增加记忆检查脚本。

验证：

- 待运行 `node scripts/check-dev-memory.mjs`。

遗留：

- 外部 skill 清单和安装脚本需要联网，当前审批系统超时，暂未安装。
- 后续可根据 `TOOLING_BACKLOG.md` 补装。

## 2026-06-12：单本 EPUB 导入书库入口

变更：

- 新增 `library.importBytes` 桥接协议。
- `reading-core::library` 支持从 EPUB 字节导入对象仓库。
- 书库 UI 新增“导入 EPUB”按钮，支持多选导入。

验证：

- `node scripts/check-arch.mjs` 通过。
- `git diff --check` 通过。

遗留：

- `npm run build` 因缺 `node_modules` 未完成。
- `cargo test --workspace` 因 crates.io 网络不可用未完成。


## 2026-06-12：新增项目本地技能包与开发日志工具

变更：

- 新增项目本地技能包与开发日志工具

修改文件：

- `AGENTS.md`
- `CLAUDE.md`
- `package.json`
- `scripts/check-dev-memory.mjs`
- `scripts/dev-note.mjs`
- `.codex/skills/project-memory-maintainer/SKILL.md`
- `.codex/skills/dev-workflow-runner/SKILL.md`
- `.codex/skills/architecture-guard/SKILL.md`

验证：

- node scripts/check-dev-memory.mjs 待运行
- node scripts/check-arch.mjs 待运行

未验证/阻塞：

- 官方 skill 联网安装审批超时

下一步：

- 联网正常后继续安装 curated skills
- 每轮开发使用 dev-note 追加 DEV_LOG

## 2026-06-12：安装全局 skills 并补齐依赖验证

变更：

- 从 `openai/skills` curated 包安装全局 skills：`define-goal`、`playwright`、`screenshot`、`security-best-practices`、`security-threat-model`、`security-ownership-map`、`notion-knowledge-capture`、`notion-research-documentation`、`notion-spec-to-implementation`。
- 通过 `npm.cmd install` 补齐前端依赖。
- 通过放宽 Cargo HTTP 低速阈值完成 Rust 依赖下载。

修改文件：

- `docs/dev-memory/TOOLING_BACKLOG.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- `npm.cmd install` 通过，0 vulnerabilities。
- `cargo test --workspace` 通过，reading-core 16/16。
- `npm.cmd run build` 通过。

未验证/阻塞：

- 全局 skills 需要重启 Codex 后才能自动发现。
- `gh-address-comments` / `gh-fix-ci` 未安装，本机缺 GitHub CLI。

下一步：

- 重启 Codex 后确认新 skills 是否出现在可用列表。
- 需要 GitHub PR 自动化时再安装 `gh` 并补装 GitHub skills。

## 2026-06-12：整理 v0.3 到 v1.0 开发大纲

变更：

- 整理 v0.3 到 v1.0 开发大纲

修改文件：

- `docs/dev-memory/DEVELOPMENT_OUTLINE.md`
- `docs/dev-memory/README.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `AGENTS.md`
- `CLAUDE.md`
- `scripts/check-dev-memory.mjs`

验证：

- node scripts/check-dev-memory.mjs 待运行
- node scripts/check-arch.mjs 待运行

未验证/阻塞：

- 无

下一步：

- 提交当前基线
- 实机验证本地书库导入 EPUB
- 继续封面提取

## 2026-06-12：补完 v0.3 书库封面与扩展元数据

变更：

- EPUB 导入时提取 OPF 封面图并写入 `library/covers/<bookId>.<ext>`。
- 入库时回填 `books.cover_path`。
- 从 OPF 提取 `language`、`description`、`series`、`series_index`，兼容 Calibre `calibre:series` 与 EPUB 3 `property` 写法。
- 同步 `BookInfo.metadata` TypeScript 协议字段与桥接协议文档。

修改文件：

- `crates/reading-core/src/epub_parser.rs`
- `crates/reading-core/src/library.rs`
- `src/platform/protocol.ts`
- `docs/resource-library-plan/8_桥接协议_v0.1.md`
- `docs/dev-memory/NEXT_ACTIONS.md`

验证：

- `cargo test -p reading-core` 通过，18/18。
- `cargo test --workspace` 通过。
- `npm.cmd run build` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-arch.mjs` 通过。
- `git diff --check` 通过。

未验证/阻塞：

- 仍需实机运行 `npm.cmd run tauri dev` 验证封面展示、导入反馈和桌面端文件权限。

下一步：

- 跑全量 `cargo test --workspace`、`npm.cmd run build`、项目记忆检查和架构检查。
- 优化批量导入进度与失败报告。

## 2026-06-12：书架 UI 展示与导入反馈收口

变更：

- 书架卡片展示已入库封面、系列、系列序号和语言字段。
- 书籍描述进入卡片悬停提示，避免压缩书架信息密度。
- 单本/多本 EPUB 导入与 Calibre 批量导入统一使用导入汇总组件。
- 导入失败时保留失败条目和错误原因，支持展开查看前 20 条。

修改文件：

- `src/main.ts`
- `src/styles.css`
- `docs/current-project/10_开发状态_2026-06-12_Codex.md`
- `docs/dev-memory/PROJECT_MEMORY.md`
- `docs/dev-memory/DEVELOPMENT_OUTLINE.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- `cargo test --workspace` 通过。
- `npm.cmd run build` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-arch.mjs` 通过。
- `git diff --check` 通过。
- Playwright 浏览器冒烟通过：`http://127.0.0.1:5173/` 初始页和书库弹层可打开，浏览器环境正确显示 Tauri 降级提示；仅有 `/favicon.ico` 404。

未验证/阻塞：

- 仍需 `npm.cmd run tauri dev` 验证真实本地 EPUB 导入、封面文件 URL、桌面端文件权限。

下一步：

- 提交当前本地书库 v0.3 基线。
- 进行 Tauri 桌面实机冒烟。
- 更新 v0.3 状态文档。

## 2026-06-12：补齐封面本地文件加载配置

变更：

- 为 Tauri 启用 asset protocol，并将 scope 限定为 `$APPDATA/**`。
- 为 `tauri` Rust 依赖开启 `protocol-asset` feature。
- `index.html` 增加 data favicon，消除浏览器冒烟里的 `/favicon.ico` 404。

修改文件：

- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `Cargo.lock`
- `index.html`
- `docs/current-project/10_开发状态_2026-06-12_Codex.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- 第一次 `cargo test --workspace` 暴露缺少 `protocol-asset` feature，已修复。
- 修复后 `cargo test --workspace` 通过，reading-core 18/18。
- `npm.cmd run build` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-arch.mjs` 通过。
- `git diff --check` 通过。
- Playwright 浏览器冒烟通过：`http://localhost:3000/` 初始页正常，console 无 error/warning。

未验证/阻塞：

- 仍需 Tauri 桌面实机确认 app data 下的封面文件能通过 asset protocol 显示。

下一步：

- 提交当前本地书库 v0.3 基线。
- 运行 `npm.cmd run tauri dev` 做桌面实机冒烟。
