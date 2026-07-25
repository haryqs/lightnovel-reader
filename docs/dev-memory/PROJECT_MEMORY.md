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

当前主线是 v0.7 桌面插件来源收口：QuickJS、完整必选方法试跑、正式 `source.*` 来源/书库流程、
真实离线 Tauri smoke、每域限速、official-free 源站条款确认，以及只允许 `public_domain/open_license`
EPUB 的 `source.acquire` 本地 asset 获取闭环均已落地。官方仓库 Ed25519 包字节验签与首批正式发布 keyring 已落地，
官方索引现强制要求 `lnr-plugin-2026-01` 或后续受信 key 的包签名。首个 Gutenberg `0.1.0` 候选仓库已从 zip
内真实 manifest 生成索引、由正式私钥签署并在仓库外暂存；正式 updater NSIS 与 `.sig` 也已生成并组装为统一
`v0.3.1` 候选，尚未公开上传。正式候选的 NSIS 安装、启动、卸载及默认应用数据逐文件保留已经通过；
下一步是在正常公网 DNS 下复验 Gutenberg，并决定是否把测试示例提升为正式来源。

2026-07-21 起，所有正式分发入口增加发布信任门：官方插件强制验签、插件 Ed25519 公钥 keyring 与 Tauri updater
公钥必须同时配置，缺一即阻断打包；开发构建保持可用。插件包签名和应用更新签名属于两个独立信任域。

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
- 桌面 QuickJS 插件运行时，含 `host.http/html/kv/log`、Promise 契约、DTO 校验、章节 HTML 清洗与完整流程试跑。
- 正式插件来源流程：启用来源列表、分页搜索、书籍/章节读取、纯文本章节预览，以及用户显式收藏远程来源记录。
- 合法插件获取流程：可选 `acquire` 返回 EPUB 提案；宿主复核授权/域名，经共享限速/SSRF 下载器获取并验证 EPUB，
  再把远程 edition 转为 `cached` asset；`official_free/user_declared` 保持外链/临时预览。
- 官方插件包信任链：索引校验 keyId，预览与安装分别下载、核对 SHA-256，并对 zip 原始字节做 Ed25519 验签；
  私钥不进入仓库，`lnr-plugin-2026-01` 公钥已进入编译内 keyring，unsigned 官方条目不再允许。

近期状态：

- 2026-07-25：新增官方插件发布准备/独立验收工具。准备工具从 zip 内唯一 manifest 生成索引并计算
  SHA-256/大小；签名工具可强制核对私钥对应公钥；验收工具只用编译内公钥复核全部包。
  首个 `gutenberg-test@0.1.0` 候选已签名并指向未来 `v0.3.1` GitHub Release，当前只在仓库外暂存。
  同时新增带安全密码提示的 updater 构建脚本与 `latest.json` 生成器；正式 updater 构建仍待维护者交互输入密码。
- 2026-07-25：首次 updater 签名演练确认私钥密码有效、release 编译成功，但可选 MSI 暴露两个独立问题：
  WiX 默认 1252 不能编码中文，以及本机 Windows Installer 服务不可访问。WiX 已改为 `zh-CN`；
  updater 主路径收窄为 NSIS，NSIS `--no-sign` 已通过并缓存官方工具，待维护者重新输入密码生成 `.sig`。
- 2026-07-25：NSIS 交互构建成功后，签名阶段发现 Tauri build 与独立 signer CLI 的环境变量支持不同：
  build 读取 `TAURI_SIGNING_PRIVATE_KEY`，而原脚本只设置了 signer 支持的
  `TAURI_SIGNING_PRIVATE_KEY_PATH`，因此误报没有私钥；脚本兼容设置两者后正式构建成功，生成 432 字节
  `.sig`。NSIS、签名、`latest.json`、正式插件索引与 zip 已合并到仓库外统一候选，尚未上传。
- 2026-07-25：统一候选 NSIS 静默安装、首次启动和静默卸载通过；安装目录/快捷方式已清理，
  `%APPDATA%\com.lightnovel.reader` 下 `reader.db` 与 `library.sqlite` 数量、长度和 SHA-256 均保持不变。
  主机 HTTPS 可访问 Gutenberg，但 FlClash DNS `198.18.0.2` 将域名映射为 fake-IP `198.18.0.4`，
  实时插件流程被 SSRF 防护按设计拒绝；WLAN DNS 和 DNS-over-HTTPS 均返回真实公网 IP
  `152.19.134.47`。不放宽保留地址限制，待 FlClash 对该域名使用 real-IP 后复验。
- 2026-07-25：维护者在仓库外分别生成插件仓库 Ed25519 私钥与带密码的 Tauri updater 私钥，只提交两套公钥。
  插件 keyring 激活 `lnr-plugin-2026-01` 并开启强制验签；Tauri updater 公钥与 `createUpdaterArtifacts=true` 已配置。
  发布门现同时检查四项：强制插件验签、合法非空插件 keyring、updater 公钥和 updater 签名产物开关。
- 2026-07-20：插件仓库 WebDriver smoke 补回 npm 入口，并将本地包预览/安装失败从 warning 改为硬失败。
  当前 WebView2 已升级到 `150.0.4078.83`；即使使用微软官方精确匹配驱动，仓库 smoke 与既有来源 smoke 都在
  会话创建后发生 DevTools 断开，因此当前 GUI 自动化环境需修复后再做窗口复验，不能据离线测试宣称窗口通过。
- 2026-07-20：新增 `smoke:plugin-repository-signature` 离线发布链回归：临时生成 Ed25519 密钥和真实插件 zip，
  调用正式签名工具后验证原始字节签名、单字节篡改与错误公钥拒绝，并串联 reading-core/Tauri 下载后校验测试。
  Tauri 胶水层现有独立测试保证大小 → SHA-256 → 签名顺序及强制签名模式；临时私钥默认删除。
- 2026-07-20：官方插件仓库签名从“元数据预留”升级为真实 Ed25519 包验签。签名覆盖 zip 原始字节，
  未知 keyId、坏 Base64、坏签名均拒绝；预览和安装各自重新验签。新增外部 PKCS#8 私钥签署脚本；
  当前 `plugin_trust` keyring 为空且强制开关关闭，unsigned 索引会明确显示人工白名单 warning。
- 2026-07-20：QuickJS 开放可选 `acquire(remoteId, mode)`，新增 additive `source.acquire`。
  当前只接受 `public-domain/open-license` 插件声明的 `application/epub+zip`，下载不经过前端消息面；验证 EPUB 后进入对象仓库。
  Gutenberg 示例已增加公共版权 EPUB 提案与“获取并阅读”入口；当前环境仍因 fake-IP DNS 无法完成真实公网下载复验。
- 2026-07-20：真实 `reader.exe` 离线来源 smoke 已通过安装、搜索、详情、纯文本预览、收藏、来源记录、停用和重启；
  smoke 同时发现并修复在线结果被本地搜索防抖覆盖的 UI 竞态。
- 2026-07-20：插件 HTTP 执行器改为 app-wide 共享的精确域名调度器，同域请求间隔至少 1 秒；
  `official-free + HTTP` manifest 必须声明 HTTPS `legal.termsUrl`，安装预览必须展示并要求用户确认。

- 2026-07-20：复核后发现先前的 QuickJS 代码未被 Tauri feature 真正编译，且与 SDK 的 `export default`、
  Promise/标量参数、`HttpResponse.text()` 和 `host.html` 存在漂移。现已修正为可编译的真实运行路径，
  桌面 `plugin.testFlow` 可依次运行 `search → getBook → getChapter`；无网络夹具已验证 HTTP/HTML/KV/Promise/清洗闭环。
- 2026-07-20：插件 HTTP 执行器收紧安全边界：禁止自动重定向，解析后拒绝本机/内网/保留地址，固定经校验的 DNS 结果，
  限制 HTTP 响应体/HTML 输入/返回 JSON/日志尺寸，并让 HTTP 超时不超过当次 Runtime 截止时间。
- 2026-07-20：Project Gutenberg 真实插件示例已对齐当前阅读链接形状，并新增可重复运行的 ignored 联网 E2E。
  本 Codex 环境把 `www.gutenberg.org` 解析为 `198.18.0.15` 保留网段，因内网防护被预期拒绝；需在正常公网 DNS 的真实 Tauri 窗口复验。
- 2026-07-20：补齐并跟踪 `src/worker/reading-core-wasm/` 生成物；新增 `npm.cmd run build:wasm` 可重复生成脚本与
  `check:wasm` 构建守卫。普通干净检出不要求 Rust WASM 工具链即可完成前端生产构建；修改相关 core 实现时必须重生成产物。
- 2026-07-20：以只新增消息落地 `source.list/search/getBook/getChapter/collect`。插件搜索不自动入库；
  用户显式收藏时，宿主重新执行 `getBook`，由 `reading-core::plugin_source` 幂等写入 `source(kind=plugin)` 与远程来源记录。
  运行时同时开始校验返回 URL 的 manifest 精确域名、结果/章节数量和文本长度；章节 UI 只做纯文本预览。
- 2026-06-22：桥接协议进入 `1.0-rc.1` 冻结候选。冻结前四项审计（DTO 预留、章节预取语义、
  结构化错误码、资源通道边界）已完成；Tauri command 与官方 shell promise 错误统一到
  `BridgeError { code, message, details? }`，新增 `platformError` 表示系统浏览器/外部阅读器等平台能力失败。
- 2026-06-22：新增 `scripts/check-protocol-freeze.mjs` 并接入 `check:project` / `npm.cmd run build`，自动核对
  `PROTOCOL_VERSION`、`BridgeErrorCode`、Rust `BridgeError` 错误码与协议文档 8 的一致性。协议进入 rc 后，新增错误码或改版本必须让这条检查通过。
- 2026-06-22：v0.7 插件运行时开始落地第一块宿主侧策略骨架：`reading-core::plugin_manifest` 负责解析/校验 manifest、
  精确域名白名单、权限/能力去重、`user-declared` 明示确认与 `official-free + acquire` ToS warning；当前仍不执行插件、不新增桥接消息。
- 2026-06-22：`reading-core::plugin_package` 落地插件 zip 安装包读取骨架：安装前读取唯一 `manifest.json` 与同目录入口 `.js`，
  复用 manifest 策略并拒绝路径穿越/多 manifest/缺入口/非 UTF-8；仍不执行插件代码。
- 2026-06-22：`reading-core::plugin_store` 与书库“源插件（v0.7 预览）”面板落地安装前确认骨架：桌面壳用 Tauri dialog
  选择 zip 路径，core 预览/写入 app data 插件目录；`user-declared` 必须显式确认；仍不执行插件 JS。
- 2026-06-22：已安装插件元数据新增 `enabled` 开关，书库插件面板可启用/停用；运行时落地后必须跳过停用插件。
- 2026-06-22：插件管理闭环继续补齐：重新安装同 id 插件会先清理旧目录，书库插件面板可卸载插件；卸载只删插件目录，不碰书库数据。
- 2026-06-22：`reading-core::plugin_host` 已落地 v0.7 host API DTO 与运行前策略门：停用插件不运行、可选 capability 必须声明、`host.http` 精确域名/权限/保留头/超时门控、`host.kv` 权限与尺寸门控、`acquire` 下载/缓存只先放行公共版权/开放授权；仍不执行插件 JS。
- 2026-06-23：`reading-core::plugin_repository` 与 `plugin-sdk/repository.schema.json` 已落地官方白名单插件仓库索引骨架：校验 schemaVersion、manifest 官方资格、重复 id、HTTPS 包地址、SHA-256、包大小、源码地址和签名元数据形状；不下载、不安装、不执行，签名字段只是预留，尚未密码学验签。
- 2026-06-23：官方插件仓库下载校验安装链路开始接入 UI 与桥接协议：书库插件面板可加载 HTTPS 索引、逐条下载 zip、核对 SHA-256、预览并确认安装；安装时会重新下载再校验。当前仍不执行插件 JS，不引入 QuickJS，`official-free + acquire` 继续等 ToS/限速/用户确认门控。
- 2026-06-23：官方插件仓库 smoke 夹具生成器已补：`npm.cmd run smoke:plugin-repository-fixtures` 可生成合法插件 zip、SHA-256 与 `repository.json`；下一步仍需接测试 HTTPS server 或可信 HTTPS fixture URL 做真实窗口端到端 smoke。
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
