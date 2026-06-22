# 项目长期记忆

## 项目定位

`lightnovel-reader` 是一个开源、免费、无广告、本地优先的轻小说平台。

阅读器是平台的核心模块，但不是完整产品边界。平台目标是把轻小说相关的发现、索引、收藏、整理、
合法获取入口、阅读方式选择和阅读体验放进同一个本地优先的软件里。

核心路线：

```text
本地优先轻小说平台
+ 内置阅读器
+ 自有 SQLite 书库
+ 作品/系列/卷/版本/来源图谱
+ 合法资源入口与开放资源获取
+ 浏览器 / 内置阅读器 / 外部本地阅读器三种阅读方式
+ 可控插件生态
```

不做“内置全网轻小说正文资源”的 App，不做盗版聚合站或商业站正文镜像。

合法公共版权、开放授权、用户自有文件，以及经来源规则确认可获取的官方免费资源，可以进入站内获取/
缓存/阅读流程；受保护、未知授权或商业内容只保存元数据与官方入口。

## 当前阶段

当前阶段优先级是 v0.3 本地书库闭环。

已经具备：

- Rust EPUB 解析与 HTML 清洗。
- TypeScript 阅读引擎。
- Tauri v2 桌面壳。
- 阅读进度持久化。
- 标注与 Markdown 导出。
- 本地书库 SQLite。
- SHA-256 对象仓库去重。
- Calibre 作为兼容迁移来源。
- 插件契约文档与 SDK 骨架。

近期状态：

- 2026-06-22：桥接协议进入 `1.0-rc.1` 冻结候选。冻结前四项审计（DTO 预留、章节预取语义、
  结构化错误码、资源通道边界）已完成；Tauri command 与官方 shell promise 错误统一到
  `BridgeError { code, message, details? }`，新增 `platformError` 表示系统浏览器/外部阅读器等平台能力失败。
- 2026-06-22：新增 `scripts/check-protocol-freeze.mjs` 并接入 `check:project` / `npm.cmd run build`，自动核对
  `PROTOCOL_VERSION`、`BridgeErrorCode`、Rust `BridgeError` 错误码与协议文档 8 的一致性。协议进入 rc 后，新增错误码或改版本必须让这条检查通过。
- 2026-06-22：v0.7 插件运行时开始落地第一块宿主侧策略骨架：`reading-core::plugin_manifest` 负责解析/校验 manifest、
  精确域名白名单、权限/能力去重、`user-declared` 明示确认与 `official-free + acquire` ToS warning；当前仍不执行插件、不新增桥接消息。
- 2026-06-22：`reading-core::plugin_package` 落地插件 zip 安装包读取骨架：安装前读取唯一 `manifest.json` 与同目录入口 `.js`，
  复用 manifest 策略并拒绝路径穿越/多 manifest/缺入口/非 UTF-8；仍不执行插件代码。
- 2026-06-21：产品定位升级为“本地优先轻小说平台”。阅读器是核心模块，但平台边界包括发现、
  索引、收藏、整理、合法获取入口、来源记录、阅读方式选择与未来插件生态。后续 UI 应提供
  浏览器 / 内置阅读器 / 外部本地阅读器等明确阅读方式。
- 2026-06-21：阅读方式选择第一版与本地偏好已落地。书架卡片提供内置、外部、获取、浏览器动作；
  书库标题栏可选择默认阅读方式，偏好写入 `localStorage`，卡片点击和主按钮按可用动作自动回退。
- 2026-06-21：合法资源获取后的打开动作已统一第一步：青空 `public_domain` 与 OPDS `open_license`
  EPUB 获取完成后都会按默认阅读方式在内置/外部阅读器间选择；OPDS feed 面板按钮改为“获取并阅读”。
- 2026-06-21：`npm.cmd run smoke:opds` 已补验“获取并阅读”真实链路：Gutenberg OPDS → Pride and
  Prejudice → 下载入库 → 进入阅读态；同时修复 OPDS 获取后刷新书库覆盖阅读器的问题。
- 2026-06-21：OPDS `open_license` 条目已持久化独立 `acquisitionUrl`（`source_record.acquisition_url`），
  与 `remoteUrl` 官方/来源页面外链分离；书架远程 OPDS 条目可直接“获取”并按阅读偏好打开，命令层仍强制
  `rightsStatus=open_license`。
- 2026-06-21：`npm.cmd run smoke:opds` 已扩展并通过“加入书架 → 持久化 acquisitionUrl → 从书架卡片获取并阅读”
  的真实链路，样例为 Gutenberg / Pride and Prejudice。
- 单本/多本 EPUB 直接导入到书库已完成。
- 本地文件夹 EPUB 批量导入入口已完成。
- 封面提取与 `books.cover_path` 回填已完成。
- `language`、`description`、`series`、`series_index` 元数据提取已完成。
- 书架 UI 已展示封面、系列、语言，并支持批量导入失败详情。
- 书库导入主路径已调整为“EPUB / 文件夹优先，Calibre 仅作为更多导入来源里的迁移入口”。
- 已安装 `tauri-driver` 与匹配 Microsoft Edge WebDriver，`npm.cmd run smoke:tauri` 可做真实 Tauri 壳自动冒烟。
- 已新增 `npm.cmd run smoke:p0`，用隔离 app data 目录在真实 Tauri 壳内验证路径版 EPUB 导入、重复导入、封面/元数据、书库开书、章节读取、进度/标注保存与重启恢复。
- 前端已完成一轮克制的轻小说/二次元书架视觉：品牌标识、漫画线稿底纹、书脊式卡片、书库空状态和默认 light 主题。
- 前端已引入原创动漫角色与雨后书街风景插图，做成低透明动态背景/空状态插图；阅读正文时动态图层会淡出。
- 已新增 `npm.cmd run package:beta`，可生成 Windows 便携测试包与 `LightNovel Reader Launcher.cmd` 启动器。
- 已新增 `npm.cmd run installer:web`，可生成 `LightNovelReaderSetup.exe` Web 下载安装器；公网发布时必须嵌入 HTTPS zip URL 与 SHA-256。
- 已新增持久化解析缓存、SQLite 迁移框架、章节 HTML 安全清洗（防 XSS）；reading-core 测试 49 个全过。
- v0.4 进行中：标注 JSON 导出 + 跨元素高亮 + 稳健定位（已合并）；封面缩略图（迁移 v2 + image 依赖，待合 PR）。
- v0.5 在线元数据主线已合并到 `main`：AniList / 青空文库 / 小説家になろう / Bangumi 均接入在线找书；青空公共版权条目可 acquire，Bangumi / なろう 只做元数据 + 外链；`catalog_fts` 已覆盖本地资产与远程 metadata 条目搜索。
- v0.3.1 三套自动冒烟全绿（`smoke:tauri`/`smoke:p0`/`smoke:p1`，覆盖开书/翻页/划词高亮/进度+标注重启恢复/真实 Calibre 读取）+ 真实 NSIS 安装器/卸载器装卸验证通过（≈7.4MB）。
- 2026-06-17 已补过真实 Tauri 窗口里的原生文件/文件夹选择对话框冒烟，并已通过 `npm.cmd run package:beta` 生成 `dist-beta/lightnovel-reader-v0.1.0-release-windows-x64.zip`。发布与测试统一见 `docs/current-project/发布与测试.md`。

## 不可变约束

- 离线优先。
- AGPL / Copyleft。
- 静态客户端优先，不做 SaaS。
- 不内置盗版源。
- 不绕过付费、登录、DRM。
- 不把“开源免费”当作版权免责。
- 多端终局使用系统 WebView，不自写排版引擎。

## 架构纪律

```text
reading-core(Rust)
+ reader-engine(TypeScript)
+ N 个薄平台壳(WebView)
```

- 前端只 import `src/platform/`，不直接碰 Tauri API。
- 业务逻辑进 `crates/reading-core`。
- Tauri command 只做平台胶水。
- 协议字段用 camelCase。
- 改协议必须同步代码和 `docs/resource-library-plan/8_桥接协议_v0.1.md`。

## 版权与资源边界

资源最大化靠：

- 元数据。
- 官方入口。
- OPDS。
- 公共版权。
- 开放授权。
- 用户本地文件。
- 用户私有源。

不靠：

- 盗版正文聚合。
- 自动抓取商业站正文。
- 插件市场分发高风险源。
