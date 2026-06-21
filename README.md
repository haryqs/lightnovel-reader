# LightNovel Reader

本地优先、无广告、面向轻小说阅读体验的桌面阅读器。当前实现基于 **Tauri v2 + TypeScript reader-engine + Rust reading-core + SQLite**，目标是先把“本地书库、稳定阅读、合法来源入口、可打包分发”打牢，再继续做外观与多端扩展。

## 当前能力

- EPUB 阅读：目录、分页阅读、主题、字号、进度恢复、标注与 Markdown/JSON 导出。
- 自有本地书库：单本/多本 EPUB 导入、文件夹导入、对象仓库去重、封面/缩略图、系列/语言/简介元数据。
- Calibre 迁移入口：只作为已有电子书库的迁移来源，不作为产品核心依赖。
- 在线找书：AniList、Bangumi、なろう、青空文库等元数据/合法入口；商业或受版权保护内容只跳官方外链。
- OPDS：添加目录源、浏览/搜索 feed、导入远程 metadata、下载开放授权 EPUB。
- 公共版权站内阅读：仅青空文库 `public_domain` 条目可获取正文并转为本地可读资产。
- 远程条目整理：本地条目与远程 metadata 手动关联，保留阅读进度和标注锚点。
- 分发脚本：便携测试包、Web 下载器安装器、Tauri NSIS 安装包配置。

## 合规边界

本项目不内置盗版正文源，不绕过付费、登录或 DRM。内核连接器只做元数据、官方入口、公共版权、开放授权和用户自有文件。未来的来源插件也必须遵守用户授权与站点规则。

## 架构

```text
reader-engine(TypeScript UI/reading)
        |
        | ReaderBridge protocol
        v
Tauri shell(thin platform glue)
        |
        v
reading-core(Rust: EPUB/HTML sanitize/library/connectors/storage)
```

关键纪律：

- 前端业务代码只通过 `src/platform/` 访问平台能力，不直接 import `@tauri-apps/*`。
- 业务逻辑优先进入 `crates/reading-core`，Tauri command 只做参数搬运、HTTP、文件系统和平台胶水。
- 协议字段使用 camelCase；改协议必须同步代码和 `docs/resource-library-plan/8_桥接协议_v0.1.md`。
- 桌面端大字节优先走文件路径/资源通道，`library.importBytes` 主要是文件选择器与移动端沙盒兜底。

## 开发

```powershell
npm.cmd install
npm.cmd run dev
npm.cmd run tauri dev
```

常用验证：

```powershell
node scripts/check-arch.mjs
node scripts/check-dev-memory.mjs
npm.cmd run build
cargo test --workspace
git diff --check
```

可选冒烟与打包：

```powershell
npm.cmd run smoke:p0
npm.cmd run smoke:p1
npm.cmd run smoke:opds
npm.cmd run package:beta
npm.cmd run installer:web
npm.cmd run tauri build
```

真实 Tauri 冒烟依赖 `tauri-driver` 与匹配本机 WebView2 的 Edge WebDriver。

## 文档入口

新线程或新 AI 接手时先读：

- `AGENTS.md`
- `docs/README.md`
- `docs/dev-memory/PROJECT_MEMORY.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/工程约定与陷阱.md`
- `docs/resource-library-plan/7_终局架构_多端与插件运行时.md`
- `docs/resource-library-plan/8_桥接协议_v0.1.md`

## 当前开发线

当前主线在 v0.6：OPDS 与结构化错误码收口中。`opds.*` 与 `library.*` 命令已开始返回结构化 `BridgeError { code, message, details? }`，前端通过 `formatError` 统一展示。后续会继续迁移 `book.*`、`annotation.*`、`reading.*`，再做协议冻结审计。
