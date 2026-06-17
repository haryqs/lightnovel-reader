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

## 2026-06-12：GitHub 同步与跨机器记忆入口

变更：

- 确认 `origin/main` 已同步到 `c540c3b reader: 完成本地书库 v0.3 基线`。
- 更新状态文档，把本轮状态从“未提交未推送”改为“已提交并推送”。
- 补充跨机器记忆入口，说明回寝室后应从 `AGENTS.md`、`docs/dev-memory/PROJECT_MEMORY.md`、`docs/dev-memory/NEXT_ACTIONS.md` 接续。

修改文件：

- `docs/current-project/10_开发状态_2026-06-12_Codex.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- `git ls-remote --heads origin main` 返回 `c540c3b79812136fa97dc74c7ddfad5224d9a6d7`。

未验证/阻塞：

- 无。

下一步：

- 在另一台电脑 `git pull origin main`。
- 运行 `npm.cmd run tauri dev` 做桌面实机冒烟。

## 2026-06-12：接手机器环境验证与 Tauri dev 启动检查

变更：

- 在当前机器拉取 `origin/main`，确认本地已经是最新基线。
- 重新安装前端依赖并完成 Rust / 前端基础验证。
- 启动 `npm.cmd run tauri dev`，确认 Vite dev server 和 `reader.exe` 可启动。

修改文件：

- `docs/dev-memory/DEV_LOG.md`
- `docs/dev-memory/NEXT_ACTIONS.md`

验证：

- `git pull origin main` 通过，返回 Already up to date。
- `npm.cmd install` 通过，0 vulnerabilities。
- `cargo test --workspace` 通过，reading-core 18/18。
- `npm.cmd run build` 通过，包含 `node scripts/check-arch.mjs`、`tsc`、`vite build`。
- `npm.cmd run tauri dev` 可启动，Vite 返回 `http://localhost:3000/`，Cargo 运行 `target\debug\reader.exe`。
- `Invoke-WebRequest http://localhost:3000/` 返回 200。

未验证/阻塞：

- 当前 Codex shell 启动的 Tauri 进程没有暴露可见 Windows 主窗口句柄，窗口枚举也未发现可交互桌面窗口，因此无法自动完成真实文件选择器中的 EPUB 导入、封面显示、重复导入反馈、失败详情、关闭重开恢复进度和标注恢复验证。
- 仍需在可见桌面窗口中重新运行 `npm.cmd run tauri dev` 并做完整实机冒烟。

下一步：

- 在真实可见 Tauri 窗口中完成本地 EPUB 打开、翻页、导入、封面显示和重复导入反馈验证。
- 验证关闭重开后阅读进度和标注恢复。
- 若封面显示异常，优先检查 `src-tauri/tauri.conf.json` 的 `assetProtocol.scope` 和 `src/platform/tauri.ts` 的 `convertFileSrc`。

## 2026-06-12：调整书库导入优先级与轻小说书架气质

变更：

- 书库页主操作改为“导入 EPUB”“导入文件夹”“刷新书架”。
- 新增文件夹导入入口，复用已有 `library.importBytes`，按文件夹相对路径排序并过滤 EPUB。
- Calibre 入口降级到“更多导入来源”，文案改为“从 Calibre 迁移”。
- 空书架状态改为本地 EPUB / 文件夹优先，不再默认提示从 Calibre 导入。
- 书库页视觉调整为清爽轻小说书架风格：更鲜明的主色、封面卡片层次、轻量纸面纹理和主次按钮。

修改文件：

- `index.html`
- `src/main.ts`
- `src/styles.css`
- `docs/dev-memory/PROJECT_MEMORY.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DECISIONS.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- `npm.cmd run build` 通过，包含 `node scripts/check-arch.mjs`、`tsc`、`vite build`。
- `node scripts/check-dev-memory.mjs` 通过。
- `cargo test --workspace` 通过，reading-core 18/18。
- `git diff --check` 通过，仅有 Windows LF/CRLF 提示。

未验证/阻塞：

- 文件夹导入、Calibre 迁移入口和新书架视觉仍需真实 Tauri 桌面窗口交互验证。
- 当前会话没有可用的 in-app Browser 控制工具，未做视觉截图 QA。

下一步：

- 完成构建与项目检查。
- 在可见 Tauri 窗口中验证 EPUB 导入、文件夹导入、Calibre 迁移、封面显示和重复导入反馈。

## 2026-06-12：整理来源连接器路线供 Claude Code 审阅

变更：

- 新增 Claude Code 审阅入口文档，集中说明 Calibre / Kavita / Komga / OPDS / 元数据源在本项目中的定位。
- 明确当前建议路线：自有轻小说书库核心 + 来源连接器，而不是把某个成熟系统作为唯一底座。
- 增加 Claude Code 审阅问题清单和输出格式，方便后续直接交给另一个 AI 评审。

修改文件：

- `docs/current-project/11_来源连接器与轻小说平台路线_Claude审阅稿.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- `node scripts/check-dev-memory.mjs` 通过。
- `git diff --check` 通过，仅有 Windows LF/CRLF 提示。

未验证/阻塞：

- 本轮是文档整理，未重新做 Tauri 桌面实机冒烟。

下一步：

- 把该审阅稿交给 Claude Code，要求先做路线审阅，再决定是否改 schema / 协议 / UI。

## 2026-06-13：P0 桌面冒烟启动尝试仍受可见窗口环境阻塞

变更：

- 按 Claude Code 审阅结论继续执行 P0，把真实 Tauri 桌面冒烟作为 v0.3.1 发布前唯一阻塞。
- 生成临时手测 EPUB 样本到系统临时目录：`%TEMP%\lightnovel-reader-smoke-epubs`。
- 通过可见 PowerShell 启动 `npm.cmd run tauri dev`。

修改文件：

- `docs/dev-memory/DEV_LOG.md`

验证：

- `npm.cmd run tauri dev` 可启动 Vite 与 `reader.exe` 进程。
- `Invoke-WebRequest http://localhost:3000/` 返回 200。

未验证/阻塞：

- 当前 Codex 会话仍无法枚举到可见 PowerShell 或 Tauri 主窗口句柄，`reader.exe` / PowerShell 的 `MainWindowHandle` 均为 0，User32 可见窗口枚举也没有发现目标窗口。
- 因此本轮无法由 Codex 自动完成打开 EPUB、翻页、导入 EPUB/文件夹、Calibre 迁移、封面显示、重复导入反馈、关闭重开恢复进度和标注恢复。
- 未进行 v0.3.1 打包；按队列规则，实机冒烟未通过前不打测试版。

下一步：

- 需要在真实可交互桌面会话中手动运行 `npm.cmd run tauri dev` 并完成 P0 冒烟清单。
- 冒烟通过后再收口 v0.3.1 状态文档并打包测试版。

## 2026-06-13：固化 v0.3.1 手动冒烟样本与清单

变更：

- 新增 `scripts/new-smoke-epubs.ps1`，用于生成 v0.3.1 桌面冒烟 EPUB 样本。
- `package.json` 增加 `npm.cmd run smoke:fixtures`。
- 新增 `docs/current-project/13_v0.3.1_桌面冒烟清单.md`，记录真实 Tauri 窗口冒烟步骤、通过标准和结果模板。
- `NEXT_ACTIONS.md` 的 P0 增加样本生成命令与清单入口。

修改文件：

- `scripts/new-smoke-epubs.ps1`
- `package.json`
- `docs/current-project/13_v0.3.1_桌面冒烟清单.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- `npm.cmd run smoke:fixtures` 通过，生成单本、重复样本和第二卷文件夹样本。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过，reading-core 18/18。
- `node scripts/check-dev-memory.mjs` 通过。
- `git diff --check` 通过，仅有 Windows LF/CRLF 提示。

未验证/阻塞：

- 本轮仍不能替代真实可见 Tauri 桌面窗口冒烟。

下一步：

- 运行样本生成和基础检查。
- 在真实桌面窗口中照 `13_v0.3.1_桌面冒烟清单.md` 完成 P0。

## 2026-06-13：Claude Code 路线审阅与文档修订

变更：

- 完成文档 11 的路线审阅：认可「自有书库核心 + 来源连接器」路线；最大风险定位为
  schema 演进时机（单表形状不能冻结进协议 1.0）。
- 新增 `docs/current-project/12_Claude路线审阅意见_2026-06-13.md`（10 个审阅问题
  的逐条回答 + 自研边界结论 + 优先级确认）。
- `DECISIONS.md` 新增三条决策：schema 实体模型迁移定于 v0.5；自研边界三层划分
  （自研组织层 / 复用原料层 / 不碰无底洞）；协议 v0.5 冻结前演进预留。
- 文档 8：新增设计原则 5「消息面只传引用，大字节走资源通道」；壳映射表补
  Web(远期/WASM) 行；新增 v0.5 冻结前检查清单（四项，未完成不冻结）。
- 文档 9：插件契约改为「必选三函数 + 可选 capability（browse/resolveUrl/
  fetchMetadata/acquire）」分层；能力限制矩阵明确由宿主 Rust 侧强制；
  待决问题 1（browse）标记已决；user-declared 插件安装加明示确认要求。
- 文档 3：Source Adapter 接口与文档 9 契约统一；状态模型标注 v0.5 正交拆分方向
  （asset.availability × source_record.rights_status）；能力矩阵标注宿主强制。
- `NEXT_ACTIONS.md`：审阅任务标记完成；P2 增加性能项（封面缩略图/并行导入/
  持久化解析缓存）；P3 增加实体迁移、迁移框架、DTO 终稿、远程条目 UI 规范。

修改文件：

- `docs/current-project/12_Claude路线审阅意见_2026-06-13.md`（新增）
- `docs/dev-memory/DECISIONS.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`
- `docs/resource-library-plan/3_在线资源接入设计.md`
- `docs/resource-library-plan/8_桥接协议_v0.1.md`
- `docs/resource-library-plan/9_插件契约_v0.1.md`

验证：

- `node scripts/check-dev-memory.mjs`、`node scripts/check-arch.mjs`、
  `npm.cmd run build`、`git diff --check` 见本轮收工记录（纯文档改动，未动代码）。

未验证/阻塞：

- 实机冒烟（P0）仍未完成，依旧是 v0.3.1 测试版的唯一发布阻塞。

下一步：

- Codex 接手：先完成 P0 实机冒烟，再按 NEXT_ACTIONS P1 收口 v0.3.1。

## 2026-06-13：打包预配置 + UI 减法（简明大方方向）

变更（打包）：

- `src-tauri/tauri.conf.json`：productName `reader → LightNovel Reader`；version
  `0.1.0 → 0.3.1`；窗口标题改中文友好名、默认尺寸 1100×760、加 minWidth/minHeight/center；
  bundle 增加 publisher / copyright / category / short&longDescription；
  新增 `windows.nsis`（installMode=currentUser 免管理员、中英双语、显示语言选择器）。
- identifier 暂保留占位值（改它会变更 $APPDATA 路径、迁移现有书库），已在 NEXT_ACTIONS
  P0.5 标为发版前待决。

变更（UI，方向：美观 / 简明大方 / 直观明了，做减法不回滚结构）：

- body 背景：移除铺满全屏的漫画网格线，改为两道极淡暖角光晕，正文背后纯净。
- 书库 `.library-view`：4 层叠加背景（彩条+网格线+斜渐变+底色）减为纯底色 + 顶部一道
  极淡风景氛围；去掉持续 drift 动画。
- 书卡 `.book-card`：移除右上角三角、左侧竖线、左色条、毛玻璃；改为干净卡片、
  圆角 12px、hover 上浮 3px，封面为视觉主角。
- 空状态 `.empty-panel`：去掉斜纹底，改干净表面 + 圆角 14px。
- 按钮：`.btn-primary` 收为单一主色渐变（去双色重叠）；新增 `:focus-visible`
  键盘焦点环（可访问性 / 直观）；active 态加淡色底。

修改文件：

- `src-tauri/tauri.conf.json`
- `src/styles.css`
- `docs/dev-memory/NEXT_ACTIONS.md`（新增 P0.5 打包清单）
- `docs/dev-memory/DEV_LOG.md`

验证：

- `npm.cmd run build` 通过；`node scripts/check-arch.mjs`、
  `node scripts/check-dev-memory.mjs`、`git diff --check` 见本轮收工。
- UI 为纯 CSS / 配置改动，未改任何 JS 钩子（class/id 全部保留），不影响行为。

未验证/阻塞：

- `tauri build` 实机打包与安装/卸载验证需在可见桌面 + Rust 环境执行（Codex 接手）。
- UI 视觉仍需真实 Tauri 窗口与多本封面下肉眼确认。

下一步：

- Codex：P0 冒烟 → P0.5 `tauri build` 出安装器 → 验证安装/卸载 → 发 v0.3.1 测试版。

## 2026-06-13：SQLite 迁移框架就位（v0.5 实体模型前置）

背景：Codex 限额，Claude 接手在无 GUI、纯 cargo test / npm build 可验证范围内推进。
落实 DECISIONS.md 2026-06-13「应该改 #5」——v0.5 单表迁实体模型的前置基础设施。

变更：

- 新增 `crates/reading-core/src/migrations.rs`：最简迁移框架。`migrations::run` 读
  `PRAGMA user_version`，按 version 升序执行未应用迁移；每条在独立 BEGIN/COMMIT 事务
  里跑完并原子推进 user_version，失败 ROLLBACK 不留半截 schema。基线迁移用
  `CREATE IF NOT EXISTS`，使框架上线前的旧库（user_version 0 或 1）被幂等盖戳。
- `lib.rs` 注册 `pub mod migrations;`。
- `library.rs`：现有 SCHEMA 去掉硬编码 `PRAGMA user_version = 1`，收为 `SCHEMA_V1`；
  新增 `MIGRATIONS`（v1），`open_library` 改走 `migrations::run`。v0.5 实体模型作为
  version 2 追加此数组即可。
- `storage.rs`（标注+进度库）：新增 `MIGRATIONS`（基线 = 现有 SCHEMA），`init` 改走
  `migrations::run`；旧库 user_version=0 会被补盖到 1。
- 测试：migrations 模块 5 个（全新库/幂等重跑/增量升级/旧库盖戳/失败回滚）+
  library 版本契约 1 个。

验证：

- `cargo test --workspace` 通过，reading-core 18 → 24 全过。
- `node scripts/check-arch.mjs` / `check-dev-memory.mjs` / `npm.cmd run build` 通过。

未验证/阻塞：

- 无（纯 core 基础设施，不依赖 GUI；未改协议、未改前端）。

下一步：

- v0.5 设计实体表结构时，作为 library `MIGRATIONS` 的 version 2 脚本落地，
  把 books 行拆为 series/volume/edition/asset，annotations/reading_state 键不动。

## 2026-06-13：持久化解析缓存（开书/翻已读章提速）

背景：原章节缓存只在内存（`LoadedBook.chapters`），关书/重启即失效——每次开书都要
重新 ZIP 解压 + HTML 清洗，`parse_book_info`(OPF+NCX) 每次重跑。这是开书与翻页体感
成本的主要来源，且与排版引擎无关，属 v0.4 性能件。

变更：

- 新增 `crates/reading-core/src/parse_cache.rs`：按 bookId（内容哈希）把 `BookInfo`
  与清洗后的章节 HTML 落盘（`<cache>/parsed/v1/<bookId>/info.json` + `ch/<hrefHash>.html`）。
  内容变→id 变→缓存自动失效，无脏读；`CACHE_VERSION` 随清洗逻辑破坏性变化递增作废旧缓存。
  全部 fail-open：读返回 Option（None=未命中/损坏则重解析），写吞错，绝不让开书失败。
  只缓存正文文本；插图仍走 `reader-img://` 从内存字节实时解析，不进缓存。
- `lib.rs` 注册 `pub mod parse_cache;`。
- `src-tauri/src/lib.rs` 接线：`AppState` 加 `cache_dir`（app_data/cache）；`LoadedBook`
  加 `book_id`；`load_book_from_data` 开书时先查缓存（命中跳过解析）；`get_chapter`
  内存未命中先查磁盘缓存，再解析并双写。协议零改动（纯内部优化，不新增消息）。

验证：

- `cargo test --workspace` 通过，reading-core 24 → 29（parse_cache 5 个用例：BookInfo
  往返 / 章节往返+按书隔离 / 坏 JSON fail-open / href_key 稳定且文件名安全 / 路径带版本）。
  reader_lib（Tauri crate）编译通过，接线无误。
- `check-arch` / `check-dev-memory` / `npm.cmd run build` 通过。

未验证/阻塞：

- 提速幅度需真实 Tauri 窗口实测（开书 → 关 → 重开，二次应明显更快）；GUI 跑不了。
- 缓存目录暂无清理/上限策略（文本量小、内容寻址，孤儿缓存无害）；删书流程尚未存在。

下一步：

- 实机量一次二次开书耗时差。
- 库删书流程落地时，顺带按 bookId 清理 `cache/parsed/*/<bookId>`。

## 2026-06-13：章节 HTML 安全清洗（防 XSS）+ EPUB 解析健壮性

背景：用户外出约 5h 授权自主开发。本轮发现并修复一处真实安全缺口，并加固解析健壮性。
均无 GUI 依赖、cargo test 可验证。

安全清洗（`crates/reading-core/src/html_sanitizer.rs`）：

- 发现：正文经 `content.innerHTML` 注入主文档（reader-core.ts 注释称 iframe 实为 div），
  主文档持有 `window.__TAURI__` 且 `csp:null`；innerHTML 不执行 `<script>` 但 `on*` 事件
  处理属性会触发 → 恶意 EPUB 可调 Tauri 命令。原清洗只重建 `<img>`、移除 `<svg>`，
  未处理 script/事件属性/js: URL/iframe 等。
- 新增 `sanitize_security`：移除 script/iframe/object/embed/applet + base/link/meta；
  逐标签剥 `on*` 属性；中和 javascript:/vbscript:/data:text/html（含大小写、内嵌空白、
  以及 **HTML 实体编码绕过**——`is_script_url` 先经 `decode_entities_for_scheme` 解码
  `&#58;`/`&#x3a;`/`&colon;`/`java&#115;cript:` 再判 scheme，因为浏览器会在属性值里解码实体）。
  手写标签扫描器（`find_tag_end`/`scrub_attributes`/`scrub_one_tag`/`is_script_url`），
  UTF-8 安全，裸 `<`（如正文 `a < b`）按文本保留，正文里的实体（`&copy;`）不动。清洗先于排版改写。
- 决策与纵深防御待办（收紧 CSP、择期评估引入 ammonia）见 DECISIONS.md 同日条目。

EPUB 解析健壮性（`crates/reading-core/src/epub_parser.rs`）：

- 修复隐性 bug：`parse_container` 多 rootfile 时取了最后一个，改为取第一个（OCF 默认
  rendition），已取到不被覆盖。
- 补边界测试：首 rootfile / 缺 rootfile 报错 / spine 跳过缺失 idref / 缺标题为空串 /
  非 zip 字节优雅报错 / 空 href 报错。

验证：

- `cargo test --workspace` 通过，reading-core 29 → 47（+12 安全含实体绕过，+6 解析健壮）。
- `check-arch` / `check-dev-memory` / `npm.cmd run build` 通过。

未验证/阻塞：

- 安全清洗为字符串扫描级，非 HTML5 解析器级；ammonia 评估与 CSP 收紧待后续。

下一步：

- v0.4 安全项：评估收紧 `tauri.conf.json` 的 csp（需实机确认不破坏 reader-img:// 与内联样式）。

## 2026-06-13：v0.5 书库实体模型 schema 草案（设计）

变更：

- 新增 `docs/resource-library-plan/10_书库实体模型_v0.5_schema草案.md`：四层模型
  （series/volume/edition/asset，合并 work 进 volume.kind）+ source/source_record；
  正交状态字段（asset.availability × source_record.rights_status）；可直接粘贴为 library
  `MIGRATIONS` version 2 的建表 SQL + 回填 SQL（`asset.id = books.id` 内容哈希不变，
  标注/进度零改动）；`LibraryBook` DTO 演进（保扁平 + 加可选实体 id）；分四步迁移顺序。
- 设计不落地：迁移框架已就位，但不注册 v2、不建表，避免产品未采纳实体读写前留死表
  （见 DECISIONS.md 同日条目）。

验证：纯文档，`check-dev-memory` 通过。

下一步：

- v0.5 按草案 §7 四步实施（双写 → 切读 → UI 系列视图 → DROP books），每步 cargo test。

## 2026-06-13：跑通 P0 自动化冒烟（项目唯一发布阻塞）+ 修复样本 PNG

背景：Codex（限额前）建好了自动化冒烟工具链——`tauri-webdriver-smoke.mjs`（UI 启动）与
`tauri-p0-smoke.mjs`（P0 桥接全流程 + 重启恢复 + 解析缓存 + reader-img 图片 + 首/二开计时），
并已就地备好 `reader.exe`(debug)、`tauri-driver`、`msedgedriver 149`（与本机 WebView2
Runtime 149.0.4022.62 精确匹配）。Claude 接手把两套冒烟真正跑通。

发现并定位一处假阳性（真实调试，非产品 bug）：

- 现象：`smoke:p0` 一路通过到内联图片校验失败——`reader-img.localhost/Images/cover.png`
  服务了完整 409 字节、PNG 签名正确，但 WebView2 `naturalWidth=0` / createImageBitmap
  报 "could not be decoded"。
- 排查：在页面内 fetch 该 URL，对比 Rust 端发送字节的 SHA 与 WebView2 收到字节的 SHA
  —— **完全一致（ce2551…）**，证明 reader-img 传输零损耗、协议无 bug。
- 根因：`scripts/new-smoke-epubs.ps1` 里内嵌的占位 base64 cover.png 是一张**畸形 PNG**
  ——GDI+/System.Drawing 宽容地解成 256×320，但 Chromium/WebView2 的 libpng 严格、拒绝
  解码（chunk 结构错乱）。即“坏测试样本”，不是产品缺陷。
- 修复：用 System.Drawing 生成一张标准有效的 256×320 PNG（渐变+文字），base64 写回
  `new-smoke-epubs.ps1`。期间一度按错误假设给 reader-img 响应加了 CORS/Content-Length 头，
  确认无关后**已回退** `src-tauri/src/lib.rs` 至原始 handler（仅 Content-Type）。

结果（均在真实 Tauri 窗口经 WebDriver 验证）：

- `npm.cmd run smoke:tauri` → OK：标题/品牌/插画加载/默认主题 light/书库浮层（导入 EPUB/
  文件夹/搜索齐全、Calibre 在折叠的“更多导入来源”内）/关闭。
- `npm.cmd run smoke:p0` → OK：导入(vol1/vol2)、去重(文件夹副本判重)、元数据(语言/系列/
  卷序)、封面、library_list、library_open、get_chapter、**解析缓存落盘(info.json+ch/*.html)**、
  **内联图片解码 256×320**、进度 0.42 保存、标注保存，**第二会话重启后进度/标注/缓存全部恢复**。
- 解析缓存提速可量化：首开 11.33ms vs 二开 7.39ms。

修改文件：

- `scripts/new-smoke-epubs.ps1`（替换为有效 PNG）
- `src-tauri/src/lib.rs`（回退实验性响应头改动）
- 临时诊断脚本 `scripts/diag-reader-img.mjs` 已删除

验证：`cargo test --workspace` 47 全过；check-arch / check-dev-memory / npm build 通过。

未验证/阻塞（仍需人工，自动化覆盖不了）：

- 系统文件/文件夹选择器本身、真实翻页热区、鼠标划词创建高亮的 UI 交互、真实 Calibre 库迁移
  —— 按 `docs/current-project/13_v0.3.1_桌面冒烟清单.md` 人工补一遍后即可打包 v0.3.1。

下一步：

- 人工补齐 doc 13 里自动化未覆盖的交互项 → `npm.cmd run package:beta` 出便携测试包。

## 2026-06-13：P0/P1 全套冒烟绿 + 真实安装器/卸载器验证

背景：computer-use 桌面驱动因「应用索引会话级缓存」无法授权新装/开发应用，改用 WebDriver
驱动真实前端补齐 UI 级 P0；并通过真实安装验证打包链路。

新增 `scripts/tauri-p1-smoke.mjs`（`npm.cmd run smoke:p1`）——驱动真实前端 UI，覆盖 smoke:p0
（桥接层）未覆盖的交互：

- 书库卡片开书（真实 `openLibraryBook` 流程，非直接调命令）→ 正文渲染。
- 翻页热区 `#next-zone`/`#prev-zone` 点击，无白屏。
- 鼠标划词建高亮（构造 Selection + 派发 mouseup → 色盘 → addHighlightFromSelection）→ 入库 →
  **重开书后 `mark[data-annotation-id]` 重新渲染**。
- 真实 Calibre 库读取：`list_calibre_books("F:\\Calibre书库")` 返回 5 本（吶喊/徬徨/…）。

打包链路验证（真实安装）：

- `npm.cmd run tauri build` 产出 NSIS `LightNovel Reader_0.3.1_x64-setup.exe`（**7.37 MB**，
  印证系统 WebView2 的轻量）+ MSI。（修了 `tauri.conf.json` 的 `category` 非法值 Book→Entertainment。）
- 静默安装 `/S`：装到 `%LOCALAPPDATA%\LightNovel Reader\`，建开始菜单快捷方式，含 `uninstall.exe`。
- 静默卸载 `/S`：安装目录与快捷方式均干净移除（**安装器+卸载器闭环验证**）。

测试结论（三套自动冒烟全绿 + 安装闭环）：

- `smoke:tauri` OK（UI 启动）/ `smoke:p0` OK（桥接+恢复+缓存+图片）/ `smoke:p1` OK（UI 交互）。
- 唯一未自动覆盖：系统文件/文件夹**选择对话框本身**（原生 OS UI；其 import 逻辑已由 p0 路径版证过，
  低风险，约 20 秒人工即可）。

未验证/阻塞：

- 仅剩原生选择对话框人工点一次；之后即可 `package:beta` 出便携包发 v0.3.1。

下一步：

- 发 v0.3.1；本轮后续做文档整理归类（精简/合并/去重）。

## 2026-06-13：文档整理归类（37 → 23 篇）

变更：

- 新增 `docs/README.md`（文档地图：持久记忆 / 当前项目 / 设计蓝图 三类导航）。
- 新增 `docs/dev-memory/工程约定与陷阱.md`（沉淀原 4_开发约定 + 6_v0.2修复 + 7/8_交接 的持久部分：
  代码/协作约定、7 条工程陷阱、安全清洗注意、OCR/MuPDF 等远期点子）。
- 新增 `docs/current-project/发布与测试.md`（合并原 13/14/15：自动冒烟 + 人工项 + 打包/安装/卸载）。
- 删除 16 篇被取代/重复/一次性文档：current-project 0/2/3/4/5/6/7/8/9/10/11/13/14/15 + AI协作指南、
  resource-library-plan 6_给Claude审阅问题（早期 epubjs 期说明、v0.1 里程碑、逐机器工作流、状态快照、
  交接稿、已答复的审阅清单等）。精髓已并入上述新文档；tracked 文件可从 git 恢复。
- 交叉引用同步：`AGENTS.md` / `CLAUDE.md` 阅读顺序、`dev-memory/README`、`SESSION_TEMPLATE`、
  `NEXT_ACTIONS`、`resource-library-plan/0`、`.codex/skills/project-memory-maintainer/SKILL.md`、
  `scripts/check-dev-memory.mjs`（必需文件改为新索引与约定文档）。

整理后结构：`docs/README.md` + dev-memory(9) + current-project(3) + resource-library-plan(10)。

验证：`check-dev-memory` / `check-arch` / `npm.cmd run build` 全过；`git diff --check` 干净。

## 2026-06-13：安装 Tauri WebDriver 工具并固化自动桌面壳冒烟

变更：

- 安装 `tauri-driver v2.0.6` 到 `C:\Users\Administrator\.cargo\bin\tauri-driver.exe`。
- 下载并解压匹配当前 WebView2/EdgeCore 的 Microsoft Edge WebDriver
  `149.0.4022.62` 到
  `C:\Users\Administrator\AppData\Local\lightnovel-reader-tools\msedgedriver\149.0.4022.62\msedgedriver.exe`。
- 新增 `scripts/tauri-webdriver-smoke.mjs`：无新增 npm 依赖，通过 WebDriver 启动真实
  `target\debug\reader.exe`，验证标题、默认 light 主题、书库覆盖层、导入入口和 Calibre
  二级来源面板。
- `package.json` 新增 `npm.cmd run smoke:tauri`。
- 修正 `src/main.ts` 默认主题：无用户偏好时使用 `light`，与 `index.html` 和本轮二次元轻量风格一致。
- 更新 `docs/current-project/13_v0.3.1_桌面冒烟清单.md`、`PROJECT_MEMORY.md`、
  `NEXT_ACTIONS.md`，说明自动壳冒烟能力与人工 P0 的边界。

修改文件：

- `scripts/tauri-webdriver-smoke.mjs`
- `package.json`
- `src/main.ts`
- `docs/current-project/13_v0.3.1_桌面冒烟清单.md`
- `docs/dev-memory/PROJECT_MEMORY.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- `npm.cmd run tauri -- build --debug --no-bundle` 通过，生成
  `target\debug\reader.exe`。
- `npm.cmd run smoke:tauri` 通过：WebDriver 成功启动真实 Tauri 窗口，书库 UI 自动冒烟通过。
- `npm.cmd run smoke:fixtures` 通过，生成手测 EPUB 样本。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过，reading-core 18/18。
- `git diff --check` 通过，仅有 Windows LF/CRLF 提示。

未验证/阻塞：

- `smoke:tauri` 不自动导入 EPUB，不验证系统文件选择器、文件夹选择器、阅读进度恢复或标注恢复。
- 完整 P0 人工实机冒烟仍未完成；v0.3.1 测试版打包仍需等手工清单通过。

下一步：

- 在可交互桌面窗口按 `docs/current-project/13_v0.3.1_桌面冒烟清单.md` 完成完整 P0。
- P0 通过后收口 v0.3.1 状态文档并打包测试版。

## 2026-06-13：前端轻小说视觉升级与便携启动器

变更：

- 前端完成一轮克制的轻小说/二次元化：顶部品牌块、应用图标、漫画线稿底纹、书脊式书卡、
  书库头部徽标、可操作空状态和小屏布局保护。
- 新增 `public/app-icon.png`，复用现有 Tauri 应用图标作为前端视觉资产。
- 空状态按钮接入现有 `openLibrary` / 文件选择流程，不新增平台依赖。
- `scripts/tauri-webdriver-smoke.mjs` 扩展检查品牌块与空状态按钮。
- 新增 `scripts/package-beta.ps1` 和 `npm.cmd run package:beta`，生成 Windows 便携测试包：
  `reader.exe`、`LightNovel Reader Launcher.cmd`、`README.txt`、`VERSION.txt`、`samples\`。
- 新增 `docs/current-project/14_v0.3.1_测试版打包与启动器.md`。
- `DECISIONS.md` 记录：v0.3.1 先做便携测试包，不做自动更新器。

修改文件：

- `index.html`
- `src/main.ts`
- `src/styles.css`
- `public/app-icon.png`
- `scripts/package-beta.ps1`
- `scripts/tauri-webdriver-smoke.mjs`
- `package.json`
- `.gitignore`
- `docs/current-project/14_v0.3.1_测试版打包与启动器.md`
- `docs/dev-memory/DECISIONS.md`
- `docs/dev-memory/PROJECT_MEMORY.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- `npm.cmd run build` 通过。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run tauri -- build --debug --no-bundle` 通过。
- `npm.cmd run smoke:tauri` 通过：真实 Tauri 窗口中品牌、默认主题、空状态、书库入口和 Calibre 二级来源均正常。
- 真实 Tauri 截图已肉眼检查：书库布局无明显重叠，整体呈轻小说书架风格且不过度装饰。
- `npm.cmd run package:beta -- -Configuration debug -SkipBuild` 通过，生成
  `dist-beta\lightnovel-reader-v0.1.0-debug-windows-x64.zip`，启动器/README/VERSION/样本均存在。

未验证/阻塞：

- 尚未运行 release 版 `npm.cmd run package:beta`。
- 便携启动器未做人工双击验证。
- 完整 P0 人工实机冒烟仍未完成，v0.3.1 仍不能正式分发。

下一步：

- 在可交互桌面窗口完成完整 P0 手工冒烟。
- P0 通过后运行 release 版 `npm.cmd run package:beta`，将 `dist-beta\*.zip` 作为测试版候选。

## 2026-06-13：新增 `.exe` Web 下载安装器

变更：

- 新增 `tools/installer/LightNovelReaderSetup.cs`：Windows bootstrapper 源码，负责下载/复制 zip、
  校验 SHA-256、解压到用户目录、写启动命令并可选启动 `reader.exe`。
- 新增 `scripts/build-web-installer.ps1` 和 `npm.cmd run installer:web`。
- `.gitignore` 忽略 `dist-installer`。
- 新增 `docs/current-project/15_v0.3.1_Web下载安装器.md`。
- `DECISIONS.md` 记录：Web 下载安装器不等同自动更新系统，公网分发必须使用 HTTPS URL 与 SHA-256。

修改文件：

- `tools/installer/LightNovelReaderSetup.cs`
- `scripts/build-web-installer.ps1`
- `package.json`
- `.gitignore`
- `docs/current-project/15_v0.3.1_Web下载安装器.md`
- `docs/dev-memory/DECISIONS.md`
- `docs/dev-memory/PROJECT_MEMORY.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- `npm.cmd run installer:web` 通过，使用本地 `dist-beta\*.zip` 生成
  `dist-installer\LightNovelReaderSetup.exe` 与 manifest。
- 运行 `LightNovelReaderSetup.exe /install-dir <temp> /no-launch /no-shortcuts /quiet` 通过：
  成功校验 SHA-256、解压安装并生成 `<temp>\App\reader.exe`、README、VERSION 和样本 EPUB。

未验证/阻塞：

- 未用公网 HTTPS URL 做真实下载测试。
- 未做默认路径的人工双击安装测试。
- 完整 P0 人工实机冒烟仍未完成，安装器仍只能作为内部候选。

下一步：

- P0 通过后，先生成 release 便携 zip，再用其公网 URL 和 SHA-256 生成对外下载器。

## 2026-06-13：引入原创动漫角色与风景动态前端

变更：

- 使用内置 imagegen 生成两张原创视觉资产：
  - `public/illustrations/window-reader-mascot.png`：窗边阅读看板娘空状态插图。
  - `public/illustrations/book-town-evening.png`：雨后书街风景背景。
- `index.html` 新增环境插图层 `ambient-scene`，空状态改为角色插图 + 操作按钮布局。
- `src/styles.css` 新增低透明风景背景、角色插图卡片、慢速漂移/光粒动画、阅读态淡出和
  `prefers-reduced-motion` 降级。
- `src/main.ts` 在打开本地 EPUB 或从书库打开书籍后给 `body` 增加 `reading-active`，让动态图层淡出。
- `scripts/tauri-webdriver-smoke.mjs` 增加插图 DOM 与图片加载检查。
- `DECISIONS.md` 新增视觉资产边界：只使用原创/自有授权插图，不引用现有动漫 IP。

修改文件：

- `index.html`
- `src/main.ts`
- `src/styles.css`
- `scripts/tauri-webdriver-smoke.mjs`
- `public/illustrations/window-reader-mascot.png`
- `public/illustrations/book-town-evening.png`
- `docs/dev-memory/DECISIONS.md`
- `docs/dev-memory/PROJECT_MEMORY.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- `npm.cmd run build` 通过。
- `npm.cmd run tauri -- build --debug --no-bundle` 通过。
- `npm.cmd run smoke:tauri` 通过：真实 Tauri 窗口中插图 DOM 存在且图片加载成功。
- 真实 Tauri 截图已肉眼检查：角色/风景插图低调，空状态无重叠，按钮可读。

未验证/阻塞：

- 尚未在完整 P0 人工冒烟里验证打开正文后的阅读态淡出和长时间阅读体验。

下一步：

- P0 手工冒烟时补充确认动态插图不干扰阅读，必要时继续降低透明度或动画强度。

## 2026-06-16：v0.4 标注增强（JSON 导出 / 跨元素高亮 / 稳健定位）+ 封面缩略图

承接 v0.3.1 发版后，推进 v0.4。已分三个 PR 合入 main（#2/#3）+ 一个待合（封面缩略图）。

标注增强（PR #2 JSON 导出、PR #3 跨元素+定位，均已合并）：

- `exportAnnotationsJson`：完整结构化导出（id/kind/color/章节/anchor/note/时间戳），标注侧栏 MD/JSON 双导出。
- `computeAnchor` 改用 `cloneContents().textContent` 计偏移，与定位用的 textContent 同口径，
  修块级边界 `range.toString()` 插 `\n` 导致的跨段落锚点错位；创建走 `applyAnnotationHighlight`
  按偏移逐文本节点包裹，跨段落选择即时高亮。
- `locateAnnotationOffset`：收集所有 exact 出现位置，prefix/suffix 消歧 + start 就近，
  修正文重复文本「永远高亮第一个」错位。
- `smoke:p1` 新增端到端校验：JSON 导出拦截 blob 解析、跨元素渲染为多段 mark。

封面缩略图（本分支 feature/v0.4-cover-thumbnails，待开 PR）：

- 加 `image` 依赖（png/jpeg，fail-open）；导入时生成 ≤240×360 缩略图 `covers/<id>_thumb.png`。
- 迁移框架 **version 2**（`ALTER TABLE books ADD COLUMN thumb_path`）—— 迁移框架上线后首个真实增量迁移。
- `LibraryBook` DTO + 协议加可选 `thumbPath`；书架优先缩略图 + `loading="lazy"`/`decoding="async"`。
- 见 DECISIONS.md 2026-06-16。

验证：

- `cargo test --workspace` 47 → **49 全过**（+2 缩略图：真 PNG 生成 / 不可解码 fail-open；版本契约改为 v2）。
- 三套冒烟全绿；`smoke:p0` 新增真窗口缩略图断言；`npm build` / `check-arch` 通过。

下一步：

- 开封面缩略图 PR；v0.4 余下「并行导入（rayon）」待定（需新依赖，等用户点头）。

## 2026-06-16：v0.5-a 实体模型落地：library 迁移 v3 建 series/volume/edition/asset + source/source_record + catalog_fts，从 books 回填四层链（asset.id=books.id，标注/进度键不动）；import 双写实体链；顺手修 EPUB fixture 时间戳致去重测试 flaky

变更：

- v0.5-a 实体模型落地：library 迁移 v3 建 series/volume/edition/asset + source/source_record + catalog_fts，从 books 回填四层链（asset.id=books.id，标注/进度键不动）；import 双写实体链；顺手修 EPUB fixture 时间戳致去重测试 flaky

修改文件：

- 待补充

验证：

- cargo test --workspace 52 全过（+3 新测试：回填/双写/同系列归并）；check-arch OK；check-dev-memory OK

未验证/阻塞：

- 无

下一步：

- v0.5-b：list/search/get 改读实体表 JOIN 回填扁平 DTO，books 转只读；LibraryBook DTO 加可选 seriesId/volumeId/editionId/availability

## 2026-06-16：v0.5-b：list/search/get 读路径 LEFT JOIN 实体表，LibraryBook 新增可选 seriesId/volumeId/editionId/availability（Rust+TS 同步）；books 仍作核心字段权威读源（保 thumbPath），实体表只补新字段；兑现协议冻结清单第1条

变更：

- v0.5-b：list/search/get 读路径 LEFT JOIN 实体表，LibraryBook 新增可选 seriesId/volumeId/editionId/availability（Rust+TS 同步）；books 仍作核心字段权威读源（保 thumbPath），实体表只补新字段；兑现协议冻结清单第1条

修改文件：

- 待补充

验证：

- cargo test -p reading-core 53 全过（+1：JOIN 回填校验）；npm run build（check-arch+tsc+vite）通过

未验证/阻塞：

- 无

下一步：

- v0.5-c：书架系列聚合视图（消费 seriesId 折叠同系列卷）；远期 v0.6 DROP books

## 2026-06-16：v0.5-c：读路径锚定 edition（FROM edition JOIN volume/series LEFT JOIN asset），books 退为只读镜像；迁移 v4 thumb_path 迁到 asset；LibraryBook.filePath/fileSize 转可选；library_open 对无文件条目报错；touch_last_read 同更 asset；main.ts/protocol.ts/协议文档/DECISIONS 同步

变更：

- v0.5-c：读路径锚定 edition（FROM edition JOIN volume/series LEFT JOIN asset），books 退为只读镜像；迁移 v4 thumb_path 迁到 asset；LibraryBook.filePath/fileSize 转可选；library_open 对无文件条目报错；touch_last_read 同更 asset；main.ts/protocol.ts/协议文档/DECISIONS 同步

修改文件：

- 待补充

验证：

- cargo test -p reading-core 54 全过（+1：远程 metadata_only 条目可上书架）；cargo check --workspace 退出0；npm run build 通过

未验证/阻塞：

- 无

下一步：

- 元数据连接器（AniList/Open Library）：写 series/edition/source_record，远程条目即时上书架（availability=remote，只展示+外链）；v0.5-c UI 系列聚合可并行

## 2026-06-16：首个元数据连接器 AniList：core 新增 connectors.rs（查询构造+JSON解析+落库，纯函数可测）；壳加 reqwest + library_search_remote 命令（HTTP 传输）；桥接加 library.searchRemote + shell.openExternal；LibraryBook 加可选 remoteUrl；前端书库加'在线找书'按钮、远程卡片（虚线+需购买外链标）、点击跳官方；封面按来源 URL 直载

变更：

- 首个元数据连接器 AniList：core 新增 connectors.rs（查询构造+JSON解析+落库，纯函数可测）；壳加 reqwest + library_search_remote 命令（HTTP 传输）；桥接加 library.searchRemote + shell.openExternal；LibraryBook 加可选 remoteUrl；前端书库加'在线找书'按钮、远程卡片（虚线+需购买外链标）、点击跳官方；封面按来源 URL 直载

修改文件：

- 待补充

验证：

- cargo test -p reading-core 58 全过（+4 连接器：解析/空与异常容错/请求体/落库端到端含 remoteUrl）；cargo check --workspace 退出0；npm run build 通过；check-arch/check-dev-memory OK

未验证/阻塞：

- 无

下一步：

- 更多连接器（Open Library / 青空文库 OPDS）复用 connectors::ingest；catalog_fts 让远程条目可全文搜；远程条目去重/与本地书手动关联

## 2026-06-16：v0.5-e PR-A — 第二个连接器（青空文库）parser

背景：寝室电脑会话续做第二个连接器（青空文库，公共版权 → 首个未来可站内自由阅览的源）。
先把数据源查实再写 parser（用户「按推荐前进」）。

数据源调研结论（WebSearch/WebFetch）：

- 社区 REST API（api.aozorahack.net）非官方且实测连不上 → 弃用。
- 选**官方「全作品扩展目录」CSV**（`list_person_all_extended_utf8.zip`，aozora.gr.jp）：权威、稳定、
  ToS 干净。列含 作品ID/作品名/作品著作権フラグ(なし=公共版权)/図書カードURL/姓/名/テキストファイルURL。

变更：

- `crates/reading-core/src/connectors.rs` 新增 `aozora` 子模块：`parse_catalog_csv(csv, query, limit)`
  纯函数——**按表头名取列**（抗列序变化）、按作品名子串过滤、按作品ID去重（同作品多行/著者+译者）、
  `著作権フラグ=なし`→`public_domain`、姓+名相连为作者、site_url=図書カードURL、language=ja。
- `Cargo.toml` 加 `csv = "1"`（解析带引号 CSV，胜过手搓）。
- 决策见 DECISIONS.md 同日条目（含 PR-A/PR-B 分层与传输层待决）。

修改文件：

- `crates/reading-core/src/connectors.rs`、`crates/reading-core/Cargo.toml`、
  `docs/dev-memory/DECISIONS.md`、`docs/dev-memory/DEV_LOG.md`、`docs/dev-memory/NEXT_ACTIONS.md`

验证：

- `cargo test -p reading-core` **63 全过**（+5 青空：过滤去重rights映射 / 空查询+著作権あり / limit+子串 /
  缺列报错 / 公共版权条目上架）。check-arch / check-dev-memory OK。

未验证/阻塞：

- PR-A 只含 parser，**尚未接壳**——青空还不会出现在「在线找书」。需先定传输层：
  官方 CSV ≈13MB，壳须下载一次 + 缓存复用 + 解压（首拉 UX 需确认）。

下一步：

- 与用户确认传输/缓存策略 → 接壳（下载缓存 CSV + 并入 library_search_remote）→ 青空进「在线找书」；
- 之后 PR-B：拉公共版权正文导入为本地 asset → 站内自由阅览。

## 2026-06-16：v0.5-e PR-B — 青空接入在线找书 + 公共版权正文获取

背景：承接 PR-A（青空 catalog parser 已合 main）。用户确认采用方案 A：青空文库作为按需来源，只有用户主动选择青空搜索时才下载官方目录，避免影响只用本地书库/AniList 的用户。

变更：

- `src-tauri/src/lib.rs`：新增 `library_search_remote_source(source, query)` 与 `library_acquire_remote(id)`。AniList 旧 `library_search_remote(query)` 保留为兼容入口；青空搜索按需下载 `list_person_all_extended_utf8.zip`，在壳侧解压 CSV、缓存到 `cache/connectors/aozora/`，7 天内复用；HTTP/ZIP/缓存留在壳。
- `crates/reading-core/src/connectors.rs`：复用 PR-A parser，补 `CatalogWork` / `find_catalog_work_by_id` 以便 acquire 按作品 ID 取官方 HTML URL；`source.kind` 用 `catalog`；重复 ingest 时保留已 cached 的 `source_record.availability`。
- `crates/reading-core/src/library.rs`：`LibraryBook` 新增 `rights_status`/TS `rightsStatus`；新增 `remote_acquisition` 和 `attach_remote_html_asset`。公共版权 HTML 合成为单章 EPUB asset 写入对象仓库，挂到既有 edition，`availability=cached`，复用现有阅读/进度/标注链路。
- `src/platform/*` + `src/main.ts` + `index.html` + `src/styles.css`：在线找书加来源选择（AniList / 青空文库）；远程卡片按 `rightsStatus` 展示「公共版权经典 · 可站内读」或官方外链；公共版权点击后先 acquire 再打开阅读，非公共版权仍跳官方外链。
- `docs/resource-library-plan/8_桥接协议_v0.1.md`、`DECISIONS.md`、`NEXT_ACTIONS.md` 同步 `library.searchRemoteSource` / `library.acquireRemote` / `rightsStatus` 与青空获取策略。

验证：

- `npm.cmd run build` 通过（内含 `node scripts/check-arch.mjs` + `tsc` + `vite build`）。
- `cargo test --workspace` 通过：65 passed。
- 修复了 connectors 测试目录仅按进程号命名导致并发抢同一个 SQLite 的老 fixture 问题。

未验证 / 阻塞：

- 尚未跑 `npm run tauri dev` 真窗口实机冒烟；青空目录首次下载、公共版权 HTML 正文渲染、ruby/插图显示、非公共版权拒绝下载还需联网真窗口验证。
- 未做 `catalog_fts`，远程条目的本地全文搜索仍是后续任务。

下一步：

- 真窗口验证 AniList/青空来源切换、青空首次下载缓存、公共版权 acquire 后打开阅读、非公共版权只外链。
- 通过后推分支开 PR；若实机发现青空 HTML 图片路径/编码问题，再补 shell/core 处理。

## 2026-06-16：青空定位修正为“公共版权经典文学”，不再表述为轻小说来源

背景：用户指出青空文库不是轻小说主库，而是日本经典文学/公共版权库。确认后保留 PR-B 管线，但修正产品语义，避免把“合法站内阅览试金石”误写成“轻小说内容来源”。

变更：

- 前端来源选择从“青空文库”改为“青空文库（公共版权经典）”；公共版权远程卡片标签从“公共版权 · 可站内读”改为“公共版权经典 · 可站内读”。
- 协议文档明确 `anilist` 是轻小说/ACG 元数据入口，`aozora` 是公共版权经典文学来源；`library.acquireRemote` 当前用于公共版权经典与站内阅览管线验证。
- DECISIONS 新增定位决策：青空不作为轻小说主来源；真正 LN 主线是用户自有 EPUB、元数据/官方入口，免费全文由公共版权/开放授权/用户显式安装插件承接。
- NEXT_ACTIONS 调整后续连接器优先级：Bangumi / なろう metadata 优先；カクヨム/Royal Road/正文抓取类来源需 ToS 审核，倾向 v0.7 插件。

验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（65 passed）。
- `git diff --check` 通过（仅 Windows 换行提示）。

未验证 / 阻塞：

- 尚未跑真窗口联网冒烟；本次只是语义/UI/文档修正。

## 2026-06-16：v0.5-f なろう官方 Web 小说元数据源接入在线找书

背景：用户确认青空文库不是轻小说主来源后，继续推进更贴近轻小说/网文方向的合法来源。なろう采用官方小说 API，只做元数据与官方入口，不做正文抓取。

变更：

- `crates/reading-core/src/connectors.rs`：新增 `narou` 子模块，解析官方 API JSON（跳过 `allcount` 汇总行，映射 `ncode/title/writer/story`），生成 `rights_status=official_free`、`language=ja`、`site_url=https://ncode.syosetu.com/<ncode>/` 的远程条目；补 3 个单测（解析、空/坏 JSON、ingest 上书架）。
- `src-tauri/src/lib.rs`：`library_search_remote_source` 新增 `source=narou` 分支；壳侧用 `reqwest` GET `https://api.syosetu.com/novelapi/api/`，传 `out=json/lim/word/order/of`，HTTP 留在壳，解析/落库仍进 core。
- `src/platform/protocol.ts`、`index.html`、`src/main.ts`：`RemoteLibrarySource` 扩展为 `anilist|aozora|narou`，在线找书下拉框新增“小説家になろう（Web小说元数据）”，远程卡片复用 `official_free` 标签并点击跳官方外链。
- `docs/resource-library-plan/8_桥接协议_v0.1.md`、`DECISIONS.md`、`NEXT_ACTIONS.md` 同步三源语义：AniList 商业 LN/ACG 元数据，なろう官方 Web 小说元数据，青空公共版权经典与 acquire 管线。

验证：

- `cargo test --workspace` 通过（reading-core 68 passed）。
- `npm.cmd run build` 通过（内含 check-arch + tsc + vite build）。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `git diff --check` 通过（仅 Windows 换行提示）。
- 官方 API 轻量探测通过：`https://api.syosetu.com/novelapi/api/?out=json&lim=2&word=転生&of=n-t-w-s` 返回 HTTP 200、实际 ncode/title。

未验证 / 阻塞：

- 尚未跑 `npm run tauri dev` 真窗口联网冒烟；需后续验证 AniList/なろう/青空三源切换、なろう搜索结果落库与官方外链打开。

## 2026-06-17：v0.5-f 真窗口联网冒烟 + Windows 系统代理兼容

背景：接手 PR #14 后按交接优先级跑 `npm run tauri dev` 真窗口联网冒烟。首次在真实 Tauri 窗口中触发なろう搜索时，PowerShell `Invoke-WebRequest` 可经 Windows 用户代理访问 `api.syosetu.com`，但壳侧 `reqwest` 直连失败；本机 Internet Settings 启用了 `127.0.0.1:7890` 用户代理，WinHTTP/环境变量未配置代理。

变更：

- `src-tauri/Cargo.toml`：`reqwest` 保持 `default-features=false` + `rustls-tls`，新增 `system-proxy` feature，让壳侧 HTTP 传输尊重系统代理配置；core 仍无网络依赖。
- `Cargo.lock` 随 reqwest `system-proxy` 增加 `windows-registry` 等间接依赖。

验证：

- `npm run tauri dev` 真实窗口启动成功；通过 WebView2 CDP 连接到 `http://localhost:3000/` 的 Tauri WebView 页面执行联网 smoke。
- 在线来源下拉确认三源顺序：AniList / 小説家になろう / 青空文库。
- なろう UI 搜索 `転生` 返回远程卡片，样例《転生したらスライムだった件》显示“官方免费 · 外链”，落库为 `rightsStatus=official_free`、`availability=remote`、`remoteUrl=https://ncode.syosetu.com/n6316bn/`；对なろう条目调用 `library.acquireRemote` 被拒绝（只支持青空公共版权条目）。
- AniList 搜索 `Tanya` 返回 `official_purchase` 远程条目，样例 `Youjo Senki` 跳 `https://anilist.co/manga/94846`。
- 青空搜索 `羅生門` 返回公共版权条目；`library.acquireRemote` 成功合成为 `availability=cached` 本地可读资产，`library.open` + `chapter.get` 打开首章成功（章节 HTML 约 27k 字符）。
- 青空拒绝分流补验：`作品著作権フラグ=あり` 样例《食品の混ぜ物処理および調理の毒物（1820）》拒绝下载；公共版权但无 XHTML/HTML URL 样例《更級日記》拒绝站内阅读。

未验证 / 阻塞：

- 未额外验证默认浏览器窗口实际打开行为；本轮验证到远程卡片的 `remoteUrl`、`availability=remote`、无 `filePath`，以及非可 acquire 条目不进入正文下载路径。

下一步：

- 跑常规收工检查：`cargo test --workspace`、`npm.cmd run build`、`node scripts/check-arch.mjs`、`node scripts/check-dev-memory.mjs`、`git diff --check`。
- 若 PR #14 继续推进，更新分支并把系统代理兼容纳入 PR 描述；后续转向 Bangumi 元数据或 `catalog_fts`。

## 2026-06-17：v0.5-g Bangumi 书籍元数据源接入在线找书

背景：v0.5-f 已完成なろう官方 API 元数据源和真窗口联网冒烟后，继续扩展更贴近中文/ACG 发现链路的元数据来源。Bangumi 本轮只作为书籍 subject 元数据目录，不作为正文来源、购买来源或可 acquire 来源。

变更：

- `crates/reading-core/src/connectors.rs`：新增 `connectors::bangumi`，构造 Bangumi OpenAPI `POST /v0/search/subjects` 请求体（`type=[1]` 书籍、`nsfw=false`），解析 `id/name/name_cn/short_summary/summary/images`，落库为 `rights_status=unknown`、`availability=remote`、`remoteUrl=https://bgm.tv/subject/<id>`；补解析、空响应、请求体和 ingest 上书架单测。
- `src-tauri/src/lib.rs`：`library_search_remote_source` 新增 `source=bangumi`；HTTP POST 留在 Tauri 壳侧，设置 `Content-Type/Accept/User-Agent`，解析和落库仍交给 `reading-core`。
- `src/platform/protocol.ts`、`index.html`、`src/main.ts`：`RemoteLibrarySource` 扩展为 `anilist|bangumi|aozora|narou`，在线找书下拉新增“Bangumi（中文/ACG 元数据）”；`unknown` 远程条目标签从“远程条目 · 官方外链”收紧为“远程条目 · 外链”，避免把 Bangumi 社区目录误写成官方授权入口。
- `docs/resource-library-plan/8_桥接协议_v0.1.md` 同步四源协议说明，明确 Bangumi 只取标题/简介/封面/subject 外链。

验证：

- `cargo test --workspace` 通过（reading-core 72 passed）。
- `npm.cmd run build` 通过（内含 `check-arch` + `tsc` + `vite build`）。
- Bangumi OpenAPI 轻量探测通过：经系统代理 POST `https://api.bgm.tv/v0/search/subjects?limit=2` 返回 HTTP 200，含 `type=1` 书籍 subject。
- `npm run tauri dev` 真窗口 + WebView2 CDP 冒烟通过：在线来源下拉确认四源 AniList / Bangumi / 小説家になろう / 青空文库；选择 Bangumi 搜索 `狼与香辛料` 返回远程卡片，卡片为 `book-card-remote`，有远程封面，标签为“远程条目 · 外链”。

未验证 / 阻塞：

- 本轮没有额外验证点击 Bangumi 卡片后默认浏览器实际打开 `bgm.tv/subject/<id>`；已验证远程卡片以外链条目呈现，且 `library.acquireRemote` 代码路径仍只允许青空 `src:aozora` 公共版权条目。

下一步：

- 跑收工检查：`node scripts/check-arch.mjs`、`node scripts/check-dev-memory.mjs`、`git diff --check`。
- 若 v0.5-g 合并后继续推进，优先做 `catalog_fts`，让 remote metadata 条目进入独立目录全文搜索；不要把 Bangumi/なろう 正文抓取放进内核。

## 2026-06-17：v0.5-h catalog_fts 覆盖远程 metadata 条目搜索

背景：v0.5-d/g 接入多个在线元数据源后，远程条目已经能落库和上书架，但 `library.search` 的 ≥3 字路径仍走 `books_fts`，只覆盖本地 EPUB 镜像表；远程 metadata-only 条目只能靠短词 LIKE 兜底。

变更：

- `crates/reading-core/src/library.rs`：新增 schema 迁移 v5，重建 `catalog_fts` 为 `edition_id UNINDEXED + title/author/series_title` 的 trigram FTS 表；回填现有 `edition → volume → series` 实体条目，并添加 `edition/volume/series` 触发器保持目录索引同步。
- `library::search_books`：≥3 字搜索改走 `catalog_fts` 并回连实体读路径；本地 asset 与远程 metadata-only 条目统一可搜，短词 LIKE 路径保持不变。
- 测试更新：schema 版本断言升到 5；新增 v4→v5 远程条目回填测试；远程 metadata-only 条目测试覆盖标题/作者/系列命中以及标题/作者更新后的 FTS 同步。
- `docs/resource-library-plan/8_桥接协议_v0.1.md`：同步 `library.search` 语义为“已落库目录条目（本地资产 + 远程 metadata）”。

验证：

- `cargo test -p reading-core` 通过（73 passed）。
- `cargo test --workspace` 通过（reading-core 73 passed）。
- `npm.cmd run build` 通过（内含 `check-arch` + `tsc` + `vite build`）。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `git diff --check` 通过（仅 Windows 换行提示）。

未验证 / 阻塞：

- 未跑真窗口 UI 冒烟；本轮是 core 搜索/迁移行为变更，已由 reading-core 单测覆盖本地与远程条目搜索。

下一步：

- 提交、推送并开 PR；若 v0.5-h 合并，在线元数据侧的下一步可转向远程条目去重/本地条目手动关联，或回到发版前人工项（原生文件/文件夹选择对话框 + `npm run package:beta`）。

## 2026-06-17：发版前原生选择框冒烟与 beta 便携包

背景：v0.5-h 合并进 `main` 后，回到 v0.3.1 发版前最后的人工项：真实 Tauri 窗口里点一次系统原生文件 / 文件夹选择对话框，并在通过后生成 beta 便携测试包。

验证：

- 已同步 `main` 到 `origin/main` 最新提交（PR #17 merge commit）。
- `npm.cmd run smoke:fixtures` 生成 smoke EPUB 样本。
- `npm run tauri dev` 真实窗口启动成功；通过 WebView2 CDP 触发书库 UI 按钮，再用 Windows 原生窗口枚举确认对话框：
  - “导入 EPUB”弹出系统 `#32770` 文件对话框，标题“打开”；选择 `smoke-test-lightnovel-vol1.epub` 后，后端 `library_list` 出现 `Smoke Test Light Novel Vol.1`，`availability=local`、`rightsStatus=user_owned`。
  - “导入文件夹”弹出系统 `#32770` 文件夹对话框，标题“选择要上传的文件夹”；选择 smoke fixtures 文件夹后，后端 `library_list` 出现 `Smoke Test Light Novel Vol.2`，`availability=local`、`rightsStatus=user_owned`，Vol.1 复制本按既有去重路径处理。
- `npm.cmd run package:beta` 通过；产物：
  - `dist-beta/lightnovel-reader-v0.1.0-release-windows-x64`
  - `dist-beta/lightnovel-reader-v0.1.0-release-windows-x64.zip`

未验证 / 阻塞：

- 本轮没有重跑 NSIS 安装器 / 卸载器；该项此前已通过一次。此次只验证发版前剩余的原生选择框和 beta 便携包。

下一步：

- 寝室电脑 Codex 接手时先同步 `main`，再按 `NEXT_ACTIONS.md` 顶部交接：若发便携测试版，在目标机器重跑 `npm.cmd run package:beta`；若继续功能开发，优先做远程条目去重 / 本地条目手动关联。
- 后续继续保持边界：Bangumi / なろう 只做元数据 + 外链，不做正文抓取；`library.acquireRemote` 仍只允许青空公共版权条目。

## 2026-06-17：远程 metadata 条目手动关联本地书

背景：v0.5-h 已让远程 metadata 条目进入统一目录搜索；下一步需要处理“在线找到的条目”和“用户已导入 EPUB”描述同一本书时的去重/关联，但不能自动合并误伤阅读进度和标注。

变更：

- `crates/reading-core/src/library.rs`：新增 `link_remote_to_local(remote_id, local_id, now_ms)`，只允许把无 asset 的远程 metadata 条目关联到已有本地/缓存 asset；实现方式是把 `source_record` 从远程 `edition` 移到本地 `edition`，随后清理无 asset/无 source_record 的远程空壳。
- `library::list_books/get_book/search_books` 增加可见性条件：只显示有 asset 或有 `source_record` 的 edition，避免未来重复在线搜索写回“无来源空壳”后重新出现在书架/搜索中。
- `src-tauri/src/lib.rs` + `src/platform/*`：新增桥接消息 `library.linkRemoteToLocal` / `linkRemoteToLocalLibraryBook(remoteId, localId)`。
- `src/main.ts` + `src/styles.css`：远程卡片增加“关联本地”动作；按远程标题优先搜索本地候选，找不到再回退全部本地书；用户显式确认后再关联。
- `docs/resource-library-plan/8_桥接协议_v0.1.md`、`DECISIONS.md` 同步协议与取舍。

验证：

- `cargo test -p reading-core` 通过（74 passed；新增手动关联测试覆盖 source_record 迁移、远程空壳隐藏、重复空壳回写后仍不可见）。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run build` 通过（内含 `check-arch`、`tsc`、`vite build`）。
- `cargo test --workspace` 通过（reading-core 74 passed）。

未验证 / 阻塞：

- 尚未跑真窗口 UI 冒烟；需要后续在真实 Tauri 窗口里验证“远程卡片关联本地”交互、远程空壳消失和重复在线搜索不反弹。

下一步：

- 跑 `git diff --check` 后推送分支并开 PR；后续可继续优化候选排序/展示，但不要改成自动合并。

## 2026-06-17：远程条目手动关联真窗口冒烟脚本

背景：PR #18 已实现 `library.linkRemoteToLocal`，但还欠真实 Tauri 窗口里的 UI 交互验证。为避免只做一次性手测，本轮把验证固化为可复跑脚本。

变更：

- `src/main.ts`：书架卡片增加 `data-book-id` / `data-edition-id` / `data-availability`，远程卡片的“关联本地”按钮增加 `data-action="link-remote"`，用于稳定 UI 自动化定位，不改变业务逻辑。
- `scripts/tauri-remote-link-smoke.mjs`：新增真窗口冒烟。流程：隔离 app data → 生成/导入 smoke EPUB → 写入本地进度和标注 → 在线搜索远程 metadata（默认 AniList `Tanya`）→ 点击远程卡片“关联本地” → 确认远程空壳消失、本地书仍在、进度/标注键不变 → 重复在线搜索确认无来源远程空壳不反弹。
- `package.json`：新增 `npm.cmd run smoke:remote-link`。

验证：

- `npm.cmd run build` 通过。
- `node --check scripts/tauri-remote-link-smoke.mjs` 通过。
- `npm.cmd run tauri -- build --debug --no-bundle` 通过。
- `npm.cmd run smoke:remote-link` 通过：AniList `Tanya` 返回 `Youjo Senki`（`ed:src:anilist:94846`），成功关联到 `Smoke Test Light Novel Vol.1`；关联后本地书仍可见，目标远程空壳不可见，`get_progress` 与 `list_annotations` 仍按本地 `asset.id` 命中；重复在线搜索后目标空壳仍不出现在 `library_list`。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `cargo test --workspace` 通过（reading-core 74 passed）。
- `git diff --check` 通过（仅 Windows 换行提示）。

未验证 / 阻塞：

- 未重新跑 NSIS 安装/卸载和 `package:beta`；本轮目标是 PR #18 的远程关联 UI 冒烟，不是发布包复验。

下一步：

- 更新 PR #18 并进入 review/merge；若合并后要发便携测试版，再在目标机器跑 `npm.cmd run package:beta`。

## 2026-06-17：PR #18/#19 合并后重跑便携测试包与解压启动检查

背景：远程条目手动关联功能（PR #18）与对应真窗口冒烟脚本（PR #19）已合并进 `main`，回到发版队列，按交接要求在当前机器重新生成便携测试包并做分发前解压/启动检查。

验证：

- `git checkout main && git pull --ff-only` 后同步到 `bc49c6a`（PR #19 merge）。
- `npm.cmd run package:beta` 通过，生成：
  - `dist-beta/lightnovel-reader-v0.1.0-release-windows-x64`
  - `dist-beta/lightnovel-reader-v0.1.0-release-windows-x64.zip`
- 将 zip 解压到临时目录，确认包内存在 `reader.exe`。
- 对解压出的 release `reader.exe` 运行 `node scripts/tauri-webdriver-smoke.mjs --application <extracted reader.exe>` 通过：真实 WebView2/Tauri 窗口启动、light 主题、品牌/插图/书库入口/导入按钮/来源面板均正常。
- 临时解压目录已清理。

未验证 / 阻塞：

- 本轮没有重跑 NSIS 安装/卸载；当前发版路径是便携 zip。
- 未额外手动打开启动器 `.cmd`，但 release `reader.exe` 已通过真窗口启动冒烟。

下一步：

- 若要对外分发测试版，使用当前机器生成的 `dist-beta/lightnovel-reader-v0.1.0-release-windows-x64.zip` 作为候选；正式发出前可再做一次人工下载/解压/启动抽检。
