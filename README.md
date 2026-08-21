# LightNovel Reader

**LightNovel Reader 是一个本地优先的轻小说平台，不只是 EPUB 阅读器。**

它的目标是把轻小说相关的发现、索引、收藏、整理、合法获取入口和阅读体验放到同一个平台里。内置阅读器是核心模块，但平台还要管理作品图谱、用户自有书库、远程来源记录、开放资源获取、外部阅读方式和未来插件生态。

## 下载

- [GitHub Release：Light Novel Reader v0.7.0](https://github.com/haryqs/lightnovel-reader/releases/tag/v0.7.0)
- Windows x64 用户下载 `LightNovel.Reader_0.7.0_x64-setup.exe`；安装器为当前用户安装，不需要管理员权限。
- 项目仍处于早期公开阶段；使用前建议保留重要 EPUB 与书库数据的独立备份。

## 平台能力

- **阅读内核**：EPUB 阅读、目录、分页、主题、字号、进度恢复、标注与 Markdown/JSON 导出。
- **个人书库**：单本/多本 EPUB 导入、文件夹导入、对象仓库去重、封面/缩略图、系列/语言/简介元数据。
- **作品与来源索引**：把本地 EPUB、AniList、Bangumi、なろう、青空文库、OPDS 等来源挂到统一的 series / volume / edition / asset / source_record 模型上。
- **合法获取入口**：公共版权、开放授权、官方免费入口、官方购买/阅读链接、用户自有文件。
- **站内合法阅读**：公共版权或开放授权资源可以在明确授权与来源校验后获取正文，转为本地 cached asset 后用内置阅读器打开。
- **阅读方式选择**：每个可读条目最终应允许用户选择用内置阅读器、系统浏览器或本机其它阅读器打开。
- **远程条目整理**：远程 metadata 条目可人工关联到本地资产，保留阅读进度和标注锚点。
- **可控插件来源**：`plugin-sdk` 已有 manifest/schema/host API/官方仓库索引契约；`reading-core` 提供 manifest/权限/域名/zip/host API 策略门、QuickJS 运行时和官方包 Ed25519 验签。启用插件可作为正式在线来源执行分页搜索、书籍/章节读取，并由用户显式收藏为远程来源记录；收藏不自动下载正文，只有声明 `acquire` 的公共版权/开放授权插件可经宿主复核后把 EPUB 缓存为本地 asset。
- **可信分发与更新**：便携测试包、Web 下载器、Tauri NSIS，以及桌面端手动检查、签名下载、安装和重启入口。

## 合规边界

本项目不内置盗版正文源，不绕过付费、登录或 DRM，不自动抓取商业站正文。

内核连接器只做：

- 元数据和作品索引。
- 官方入口和外链。
- 公共版权正文获取。
- 开放授权资源下载。
- 用户自有文件和用户显式选择的本地资源。

更复杂的站点正文适配必须进入插件运行时，并由权限、域名、ToS 提示和用户显式安装来约束。平台可以提供能力边界，不能替用户或第三方来源背版权风险。

## 阅读方式

平台后续会把“打开”拆成明确的阅读方式：

- **内置阅读器**：本地 EPUB、cached asset、公共版权/开放授权正文获取后进入内置阅读器，复用进度和标注。
- **浏览器打开**：商业、受保护、未知授权或官方 Web 小说条目默认跳官方页面。
- **外部本地阅读器**：本地文件或已缓存资产可交给系统默认应用，后续支持用户配置其它阅读器路径。

当前已经具备 `library.acquireRemote`、`opds.downloadEpub`、`shell.openExternal`、`shell.openPathExternal`、本地 `library.open` 等基础能力；书架卡片已提供第一版阅读方式选择：内置阅读器、浏览器官方入口、外部本地阅读器，以及合法正文获取后阅读。书库标题栏的默认阅读方式会本地持久化，卡片点击、主按钮、青空公共版权获取和 OPDS 开放授权 EPUB 获取都会按偏好选择可用动作并自动回退。

## 架构

```text
reader-engine(TypeScript UI/reading/platform UX)
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
npm.cmd run check:license
npm.cmd run build
cargo test --workspace
git diff --check
```

仓库会跟踪 `src/worker/reading-core-wasm/` 下的浏览器 WASM 产物，因此普通的干净检出不需要 Rust WASM
工具链即可执行 `npm.cmd run build`。如果修改了 `reading-core` 的浏览器导出或 EPUB/分页实现，维护者需要先安装
`wasm32-unknown-unknown` target 和与 `Cargo.lock` 一致的 `wasm-bindgen-cli`，再重新生成：

```powershell
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.122 --locked
npm.cmd run build:wasm
npm.cmd run check:wasm
```

可选冒烟与打包：

```powershell
npm.cmd run smoke:p0
npm.cmd run smoke:p1
npm.cmd run smoke:opds
npm.cmd run smoke:plugin-repository-signature
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

## 开源许可

项目代码以 [GNU Affero General Public License v3.0](LICENSE)（SPDX：`AGPL-3.0-only`）发布。
根目录许可证正文和 npm/Cargo 包元数据由 `npm.cmd run check:license` 校验，并已接入项目检查、生产构建与正式分发入口。

## 当前开发线

当前主线已推进到协议 `1.0-rc.1` 冻结候选：Tauri command 与 shell promise 错误已统一为结构化 `BridgeError { code, message, details? }`，并由 `scripts/check-protocol-freeze.mjs` 守住协议版本/错误码/文档一致性。v0.7 桌面插件链路现已具备 manifest/权限/域名白名单、zip 安装、官方仓库 SHA-256 + Ed25519 包验签、QuickJS、`host.http/html/kv/log`、正式 `source.*` 与开放资源 EPUB 获取闭环；首批插件和 updater 公钥、Gutenberg 公网全链路、NSIS 安装/卸载及数据保留均已验证。首个公开应用版本 `v0.7.0` 已正式发布；npm、Tauri 与三个第一方 Cargo 包由版本一致性门禁防止漂移。公开五资产、`releases/latest` 清单及公开下载的安装器已复验，下一步发布首个带应用内更新入口的 v0.7.1，并以它作为后续完整用户界面更新闭环的旧版本基线。
