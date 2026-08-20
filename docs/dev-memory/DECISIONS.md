# 决策日志

> 记录影响未来开发方向的取舍。格式：日期 / 决策 / 理由 / 后果。

## 2026-07-20：官方插件签名覆盖 zip 原始字节，可信公钥编译进桌面壳

决策：`PluginPackageSignature` 的 `ed25519` 签名覆盖插件 zip 原始字节，不签索引 JSON 或哈希字符串。
`reading-core::plugin_repository` 负责 Base64/长度/keyId/keyring/密码学验证，Tauri 壳提供编译内可信公钥表。
仓库索引加载先拒绝未知 keyId；包预览和安装各自重新下载、核对 SHA-256，再独立验签。
正式发布密钥尚未配置期间，unsigned 条目只在明确 warning 的人工白名单模式下放行；任何声明签名的条目都必须真实验签。

理由：

- 对实际下载字节签名可避免 JSON canonicalization 跨语言漂移，并与安装时已有的 SHA-256/重新下载链路自然组合。
- keyring 必须来自客户端发布物而非远程索引，否则攻击者可同时替换公钥和签名。
- 预览结果可能过期，安装阶段重验才能守住 TOCTOU 边界。
- 当前还没有经过秘密管理、备份、轮换和撤销设计的正式私钥，不能为了“完成签名”把测试私钥提交进仓库。

后果：

- `ring` 与 `base64` 作为 reading-core 的 native-only 可选直接依赖；二者版本已由现有 Tauri/rustls 依赖树锁定，
  WASM 构建不携带这部分代码。
- 新增 `sign:plugin-repository`，只读取仓库外 PKCS#8 Ed25519 私钥，签署每个 zip 并输出可安全编译的公钥 Base64。
- 对外发布官方仓库前必须把公钥加入 `plugin_trust.rs`、签署全部条目并开启 `REQUIRE_OFFICIAL_PLUGIN_SIGNATURES`；
  在此之前 UI 会明确显示“未签名 · 人工白名单”。

## 2026-07-20：插件正文获取使用独立 source.acquire，只接收开放资源 EPUB 提案

决策：新增 `source.acquire(pluginId, bookUrl)`，不改变 `source.collect` 的纯元数据收藏语义。
命令重新执行插件 `getBook` 与可选 `acquire(bookUrl, 'cacheForReading')`；只有 manifest 声明 `acquire`、
授权为 `public-domain/open-license` 且提案 `mimeType=application/epub+zip` 时才继续。提案由 core 复核授权和精确域名，
Tauri 经 app-wide 每域限速、SSRF/DNS 固定、禁止重定向的同一 HTTP 执行器下载；EPUB 结构验证通过后直接写对象仓库并挂到远程 edition。

理由：

- 搜索/收藏与正文副作用需要继续分离，避免用户收藏条目时隐式下载，保持协议冻结候选的新增消息纪律。
- `AcquireProposal` 只是插件声明，不能让 JS 自报授权后直接写库；最终裁决必须留在 reading-core。
- 第一版只接受 EPUB，可复用现有 `attach_remote_epub_bytes` 和阅读链路，也避免同时设计章节聚合、增量更新与图片抓取。
- 下载字节不经过前端 JSON/IPC，符合大资源走平台资源通道并最终只返回 `LibraryBook` 引用的协议边界。

后果：

- `official-free/user-declared` 即使可以临时预览章节，也不会出现自动缓存入口；通用 ToS/限速门不等于单源正文授权。
- 插件作者要提供站内获取，必须声明 `acquire` 并返回同域公共版权/开放授权 EPUB；其它 MIME 当前明确拒绝。
- Gutenberg 示例成为首个真实联网获取夹具；当前 fake-IP DNS 环境仍只能验证拒绝路径，需正常公网 DNS 实机复验。

## 2026-07-20：正式插件来源使用新增 source.* 消息，搜索不自动落库

决策：保留 `plugin.testFlow` 作为安装管理面里的三函数诊断；正式业务新增
`source.list/search/getBook/getChapter/collect`。`source.search` 只返回候选，用户显式点击收藏后，
`source.collect` 重新执行 `getBook`，由 `reading-core::plugin_source` 用插件 id + 规范书籍 URL 的哈希作为稳定键，
幂等写入 `source(kind=plugin)` 与远程 `edition/source_record`。收藏不自动下载或缓存正文。

理由：

- 诊断命令固定取第一本/第一章，语义不适合承载分页、选择和持久化；协议冻结候选要求新增消息而非改变旧消息。
- 搜索即落库会用用户未选择的候选污染个人书库；显式收藏更符合本地优先的个人目录边界。
- 收藏时重新取详情可避免直接信任前端回传 DTO；元数据映射和稳定去重属于 core 业务逻辑，不应落在 Tauri command。
- 插件返回值仍不可信，因此正式调用同时校验 URL 属于 manifest 精确域名，并限制结果数、章节数和文本长度。

后果：

- 启用插件会进入在线来源选择器；停用后退出正式来源列表，但已收藏的来源记录不会随插件卸载而删除。
- `public-domain/open-license/official-free/user-declared` 收藏分别映射为 `public_domain/open_license/official_free/unknown`；
  这只影响元数据/外链展示，不等于允许获取正文。
- 章节 HTML 仍由 core 清洗，第一版来源 UI 进一步只展示提取后的纯文本，避免插件远程资源进入持有平台能力的主文档。
- 后续 acquire/缓存必须继续新增独立消息与权限/ToS/限速门，不能借 `source.collect` 隐式下载。

## 2026-07-20：浏览器 reading-core WASM 生成物入库，维护者显式重生成

决策：跟踪 `src/worker/reading-core-wasm/` 下 wasm-bindgen 生成的 JS、类型声明和 WASM 二进制。
普通 `npm.cmd run build` 只校验这些产物存在、文件有效且保留必要导出，不在每次前端构建中安装 Rust target 或重新编译。
维护者修改相关 `reading-core` 实现后，通过 `npm.cmd run build:wasm` 使用 `Cargo.lock` 匹配的 wasm-bindgen CLI 显式重生成。

理由：

- 浏览器入口和分页 Worker 对生成模块是静态 import；未跟踪产物会让干净检出在 TypeScript 阶段直接失败。
- 强制每次前端构建重编 Rust/WASM 会让普通 Web 开发机额外依赖 rustup、WASM target 和全局 CLI，也增加 CI 时间与网络故障面。
- 生成脚本从锁文件读取 wasm-bindgen 版本并拒绝不匹配的 CLI，可避免宏 crate 与生成器 ABI 漂移。

后果：

- 仓库增加约 569 KiB WASM 二进制和小型绑定文件，但普通检出可直接完成 Web/PWA 生产构建。
- 修改 EPUB 解析、HTML 清洗、分页或 WASM 导出时，开发者必须重生成并提交产物；存在性检查无法自动证明二进制与源码完全同步。
- `check:wasm` 接入 `check:project` 与 `build`，缺文件、空文件、无效 WASM 文件头或必要导出缺失会提前给出可执行的修复提示。

## 2026-07-20：插件 HTTP 不自动跟随重定向，并在连接前拒绝非公网地址

决策：`host.http.get` 仍由 core 的 `plan_http_get` 校验 manifest 精确域名，Tauri 执行器另外解析该域名，
只允许公网 IP，再将连接固定到已校验结果。HTTP 客户端不使用系统代理，不自动跟随 3xx；插件可读取状态和 `Location`，
但后续请求必须再走一次域名/IP 策略门。同时对 HTTP 响应体、HTML 输入、返回 JSON 和日志设置硬上限。

理由：

- 只校验初始 URL 会被跨域重定向绕过；只校验域名字符串则会被 DNS rebinding 或直接指向内网。
- 插件契约没有“自动跟随重定向”承诺，保留 3xx 响应比在壳层暗中跨越权限边界更可审计。
- QuickJS 内存上限不会限制 Rust 侧在进入 JS 之前持有的 HTTP/HTML 字节，因此必须在 host 边界单独限制。

后果：

- 需要跨域重定向的插件必须把目标域名也列入 manifest，并显式发起第二次 `host.http.get`。
- 本地开发服务、内网源、使用保留网段 fake-IP DNS 的环境会被拒绝；当前优先保持安全默认，不为测试环境增加隐式豁免。
- 若未来产品确需代理/fake-IP 兼容，必须设计可见、可审计的用户授权和目标级策略，不能直接删掉内网防护。

## 2026-06-27：QuickJS 引擎选 rquickjs（绑定 quickjs-ng），每次调用一次性 Runtime

决策：v0.7 插件运行时 QuickJS 集成选择 `rquickjs` crate（绑定 quickjs-ng 社区 fork），每次方法调用创建一次性 `Runtime`+`Context`，跑完即弃。沙箱只注入 `host.http`、`host.kv`、`host.html`、`host.log` 四个命名空间 + `URL`/`TextDecoder` 两个 polyfill，不开 `std`/`os` 模块。HTTP 执行经 `PluginHttpExecutor` trait 透传壳层（core 不直接发 HTTP）。

理由：
- `quickjs-rs` 原始绑定绑定的是 Bellard 原版 QuickJS（已停更两年），MSVC 编译有已知坑；`rquickjs` 绑定 quickjs-ng（活跃维护），自带 Promise/async 桥接省胶水层。
- 一次性 Runtime 隔离最彻底：一个插件崩不影响其他，跨调用状态强制走 `host.kv`。
- HTTP 经 trait 转发保持 reading-core "无网络"架构纪律。
- 沙箱不注入 `fs`/`net`/`process`，不开 `std`/`os` 可选模块——从引擎层杜绝文件系统和网络逃逸。

后果：
- 每次调用创建/销毁 Runtime 有固定开销（~1-3ms），后续若成为瓶颈可引入 Runtime 池化。
- QuickJS 内存上限建议 64MB，需要验证实际插件场景（HTML 解析特别吃内存）。
- `PluginHttpExecutor` trait 给壳层增加了一个实现点，但保持了 core 的可单测性。

## 2026-06-27：采用 Hermes 三层架构（Hermes + Claude Code + OpenCode）进行开发

决策：Hermes 担任 Tech Lead/Orchestrator（DeepSeek v4 Pro，~免费），Claude Code -p 处理复杂架构/安全审查（Pro 额度），OpenCode 按任务复杂度分层模型处理日常实现/测试/文档（OpenRouter，~$0.01-0.30/task）。

理由：
- Claude Pro 额度有限，不应消耗在日常编码和测试编写上
- OpenCode 支持多 provider 切换，便宜模型做小事，好模型做中事
- Hermes 的持久化记忆 + 技能系统适合做跨 session 的 Project Lead
- DeepSeek 模型在文档/记录类任务中产生幻觉（已证实），此类任务由 Hermes 直接操作

后果：
- 开发流程增加一层"写 Spec → 委托 → 验证"的环节，但每个环节的 token 消耗显著降低
- 需要维护 AGENTS.md 作为三个 Agent 的共享上下文入口
- 弱模型（DeepSeek/Haiku）不适合做需要事实准确性的文档/记录工作

## 2026-06-23：官方仓库插件安装采用下载后哈希双校验，不信任预览缓存

决策：官方插件仓库 UI 先采用“索引校验 → 壳侧 HTTPS 下载 zip → SHA-256 校验 → core 预览 → 用户确认 → 重新下载并再次 SHA-256 校验 → core 安装”的链路。`plugin.repository.inspectPackage` 和 `plugin.repository.installPackage` 都只传 `packageUrl` 与 `packageSha256`；Tauri 壳负责 HTTP 下载与大小上限检查，`reading-core` 负责索引/包校验、预览与写入。

理由：

- 安装确认和预览之间可能隔一段时间，重新下载并校验能避免把临时内存包或过期预览当成可信安装来源。
- SHA-256 是签名系统落地前的最低完整性门槛；签名字段当前只做元数据预留，不能宣称已验签。
- 继续遵守协议“消息面只传引用，大字节走资源通道”的纪律，不把 zip blob 塞进前端 bridge。
- Tauri 负责联网，core 保持可单测的策略与落盘边界，符合现有架构分工。

后果：

- 官方仓库安装会产生两次下载；后续若要做下载缓存，必须把缓存也纳入哈希/签名验证策略。
- `official-free + acquire` 仍在官方仓库中被拒绝，直到源站 ToS、限速和用户确认门控补齐。
- 后续真正做签名验签时应接在 SHA-256 校验之后、安装预览之前，并更新本决策。

## 2026-06-23：官方插件仓库先做白名单索引校验，不做通用市场

决策：v0.7 官方插件分发先采用“白名单仓库索引”而不是开放市场。索引 schema 由
`plugin-sdk/repository.schema.json` 定义，core 侧由 `reading-core::plugin_repository`
校验：索引版本、manifest 官方资格、重复 id、HTTPS 包地址、SHA-256、包大小、源码地址与可选签名元数据形状。
索引校验不下载、不安装、不执行插件；下载安装链路后续必须继续核对 `packageSha256` 并复用
`plugin_package` / `plugin_store` 的安装前校验与用户确认。

理由：

- 官方索引是“可信候选列表”，不是第三方插件市场；先把可展示内容限定在合法、可审阅、可哈希校验的包上。
- SHA-256 校验能在签名系统未完成前提供最小完整性保护，且不引入新依赖。
- `user-declared` 与 `official-free + acquire` 都有合规风险，前者只能用户自装，后者在 ToS/限速/用户确认门控落地前不得进入官方索引的正文获取路径。

后果：

- 后续官方索引 UI/下载器必须先走 `plugin_repository`，再下载 zip，核对哈希后才进入安装预览。
- 签名字段当前只是预留元数据，不能对外宣称已经完成密码学验签；真正验签前需要单独实现与记录。
- 插件生态继续保持“官方白名单”和“用户自装”两条视觉与合规边界。

## 2026-06-22：插件 host API 先做 Rust 策略门，不放行 official-free 正文缓存

决策：v0.7 插件运行时继续按“先宿主策略，后 JS 执行”推进。本轮在 `reading-core::plugin_host`
落地 host API DTO 与运行前门控：停用插件不得运行；可选方法必须声明 capability；`host.http`
必须有 `http` 权限且 URL 精确命中 manifest 域名；保留请求头由宿主忽略；`host.kv` 必须有 `kv`
权限并受 key/value 尺寸限制；`acquire` 提案只能由宿主最终裁决。下载/缓存正文的第一版只放行
`public_domain` 与 `open_license`，`official_free` 在源站 ToS、限速和用户确认门控落地前保持 metadata + 官方外链。

理由：

- 插件执行风险主要来自宿主能力，而不是 JS 语法本身；HTTP、KV、正文获取必须在 Rust 侧形成可单测硬门。
- `official_free` 只说明“官方可免费访问”，不自动等于允许批量下载、缓存或站内重分发；需要按源站 ToS 单独处理。
- 协议已处于 `1.0-rc.1`，本轮不新增桥接消息、不引入 QuickJS，避免在冻结候选阶段扩大消息面。

后果：

- 后续 QuickJS/JavaScriptCore host 必须复用 `plugin_host` 的策略函数，不能绕过到平台壳直接发 HTTP 或写 KV。
- 真正放行某个 `official_free` 源的正文获取前，必须补源站级 ToS 记录、限速策略和测试，再更新本文档。
- 插件 SDK 注释与契约文档同步为“插件返回 acquire proposal，宿主做最终下载/缓存裁决”。

## 2026-06-22：插件安装 UI 用原生文件选择器走路径，不传 zip 字节

决策：v0.7 插件安装预览引入官方 `@tauri-apps/plugin-dialog` / `tauri-plugin-dialog`，由桌面壳打开原生文件选择器取得 zip 路径；`plugin.inspectPackage` / `plugin.installPackage` 只传路径与确认布尔值，壳侧读取文件后交给 `reading-core::plugin_store` 校验/写入。

理由：

- 符合桥接协议“消息面只传引用，大字节走资源通道”的纪律，避免把插件 zip 字节塞进前端消息。
- 插件安装属于平台文件权限能力，用原生选择器比让用户手填路径更符合真实软件体验。
- 安装前展示权限/域名/合规声明需要真实文件路径读取，但仍不需要执行插件 JS。

后果：

- Tauri capability 需要包含 `dialog:default`。
- 插件安装命令新增在协议 `1.0-rc.1` 的“新增消息”范围内；不改变既有消息语义。
- 后续移动端可用各自文件选择器实现同一 `plugin.selectPackagePath` 语义，或新增沙盒导入能力，但不得绕过安装确认门控。

## 2026-06-22：插件分发包先采用目录 zip，安装前只读取校验不执行

决策：v0.7 插件分发格式先采用目录 zip。zip 可直接包含 `manifest.json + plugin.js`，也可外包一层目录；宿主只允许一个 `manifest.json`，`manifest.entry` 必须是同目录的单个 `.js` 文件名。安装前 core 只读取 manifest 与入口文本并做策略校验，不执行插件代码。

理由：

- zip 是最简单的跨平台分发单位，能同时服务桌面和未来移动端导入。
- 先固定包结构可以让安装 UI、权限确认、官方索引格式并行推进。
- 安装前不执行代码是插件安全边界的第一层，避免“读取包信息”阶段就引入运行时风险。

后果：

- 后续若加入签名或官方索引，也应围绕 zip 包与 manifest 做外层校验，不改变插件目录内基本布局。
- `manifest.entry` 不支持子目录入口；如未来确有需求，必须先更新 SDK schema、core 包加载器、文档和安装 UI。
- 插件包加载不等于安装完成；仍需用户确认权限/合法性后才能进入本地插件存储。

## 2026-06-22：v0.7 插件先落宿主侧 manifest 策略，不先执行插件代码

决策：v0.7 插件运行时的第一步只在 `reading-core` 落地 manifest/权限/合规策略模型：解析和校验 manifest、精确域名白名单、能力声明、`user-declared` 明示确认、`official-free + acquire` ToS warning。暂不引入 QuickJS、不新增桥接消息、不运行第三方插件代码。

理由：

- 插件生态的核心风险不是“能不能执行 JS”，而是执行前是否把域名、权限、授权性质和用户确认做成宿主侧硬门。
- 协议已进入 `1.0-rc.1`，此时不应为了插件探索扩大桥接消息面。
- 先让 SDK schema、示例 manifest 和 Rust core 校验互相牵住，后续安装 UI 与 QuickJS host API 才有稳定底座。

后果：

- `plugin-sdk/manifest.schema.json` 的 `capabilities` 与 `reading-core::plugin_manifest` 必须继续保持一致。
- 任何 acquire 型插件能力都不能只信插件返回值；宿主必须按 manifest、rights/status、ToS 与限速策略二次裁决。
- `user-declared` 插件只能用户自装，安装时必须明示确认，官方仓库/官方 UI 不得把它表现成背书来源。

## 2026-06-22：桥接协议进入 1.0-rc.1 冻结候选

决策：将 `src/platform/protocol.ts` 的 `PROTOCOL_VERSION` 从 `0.1` 推进到 `1.0-rc.1`。官方 Tauri
壳的 promise 型桥接错误统一收敛到 `BridgeError { code, message, details? }`；新增
`platformError` 覆盖系统浏览器/外部阅读器等平台能力失败。

理由：

- 冻结前四项审计已经完成：DTO 预留、章节预取语义、结构化错误码、资源通道边界。
- Tauri command 面已全部返回 `BridgeError`；`shell.openExternal` / `shell.openPathExternal` 也由 TS
  壳侧包装为结构化错误，避免协议面保留裸字符串/原生 Error 例外。
- `1.0-rc.1` 表示冻结候选而非最终冻结：后续仍可在 rc 阶段修正审计发现的小问题，但不能随意扩大或重写协议面。

后果：

- 冻结后原则：只能新增消息/新增可选字段，不改名、不删字段、不改变既有语义；破坏性变更需升大版本。
- 新增 promise 型桥接方法默认必须返回 `BridgeError` 形态；新增错误码必须同步 TS 类型、文档，若由 Rust command 发出还要同步 Rust 构造器。
- 文件 `docs/resource-library-plan/8_桥接协议_v0.1.md` 暂不改名，以保持历史链接稳定；内容和
  `PROTOCOL_VERSION` 才是版本权威。

## 2026-06-22：协议冻结前不新增章节预取消息，资源通道边界通过审计

决策：冻结前不新增 `chapter.prefetch` / `chapter.getBatch`。继续把 `chapter.get(href)` 作为唯一章节 HTML 获取消息，
允许 reader-engine 用它做有界后台预取；资源通道边界维持现状，消息面只保留 `book.open(data)` 与
`library.importBytes(data)` 两个移动/沙盒兜底大字节例外。

理由：

- `ReaderCore.preloadAroundChapter` 已在当前章加载后后台预取前一章、后一章、后两章，并通过
  `chapterInflight` 去重、`maxCachedChapters=10` 控制内存。
- Tauri/core 侧已有当前书内存章节缓存与持久化 parse cache；二次读取同章不重解析。
- 当前 P0/P1 冒烟与 release 启动候选未暴露跨章 IPC 往返为主瓶颈；新增批量消息会扩大冻结前协议面。
- 资源通道审计确认：桌面批量导入和书库开书传路径或 id，OPDS/青空下载在壳侧完成，正文图片走
  `reader-img` URL scheme，封面/缩略图走 `resource.url` 或来源 http(s) URL。

后果：

- 协议冻结清单第 2 项（批量/预取语义）和第 4 项（资源通道核对）标记完成。
- 冻结后若真机数据证明跨章翻页延迟仍不可接受，只能新增可选 `chapter.prefetch(hrefs)` 消息，
  不能改变既有 `chapter.get` 语义。
- 后续新增能力不得把整本书、图片或二进制 blob 直接塞入 JSON 消息；必须使用路径、id、URL scheme、
  HTTP 流或另行设计的流式资源通道。

## 2026-06-21：产品定位升级为轻小说平台

决策：`lightnovel-reader` 的产品定位从“轻小说/电子书阅读器”升级为**本地优先轻小说平台**。阅读器是核心模块，但平台边界包括发现、索引、收藏、整理、合法获取入口、来源记录、阅读方式选择和未来插件生态。

理由：

- 用户目标不是只打开 EPUB，而是构建一个围绕轻小说的长期个人平台。
- 已落地的实体模型、在线元数据、OPDS、来源记录、公共版权 acquire、远程条目关联，已经超出单纯阅读器边界。
- 平台化定位能统一本地书库、远程 metadata、公共版权、开放授权、官方入口和插件运行时，避免功能被零散地堆在“阅读器”外壳上。

后果：

- README、PROJECT_MEMORY、DEVELOPMENT_OUTLINE、资源书库文档统一使用“轻小说平台”表述。
- 后续 UI 不应只围绕“打开文件”，而应围绕“作品/条目/来源/阅读方式”组织。
- v0.6 后新增 v0.6.5 阶段：阅读方式选择与合法开放资源获取体验。

## 2026-06-21：合法开放资源可站内获取，但阅读方式必须由用户选择

决策：对公共版权、开放授权、用户自有资源，以及经 ToS/授权确认可获取的官方免费资源，平台可以提供正文/EPUB 获取、缓存和站内阅读能力。每个可读条目最终应提供明确阅读方式：浏览器打开、内置阅读器打开、外部本地阅读器打开。

理由：

- 合法开放资源如果只能跳浏览器，会浪费平台的阅读进度、标注、缓存和排版能力。
- 不同用户有不同阅读习惯：有人想用内置阅读器，有人想去官方网页，有人已有本机阅读器。
- “official_free” 不等于“可缓存正文”；是否能站内获取必须由 rights/status、来源能力和 ToS 共同决定。

后果：

- `library.acquireRemote` 的硬门原则保留：只有明确 `public_domain` / `open_license` / 已审核可获取来源才能下载正文。
- 商业、受保护或未知授权条目只保存 metadata 与官方入口，不提供正文抓取。
- UI 需要新增动作选择模型：`openInBrowser`、`openInBuiltinReader`、`openInExternalReader`、`acquireThenOpen`。
- 后续若新增外部阅读器能力，应通过 `src/platform/` 暴露平台命令，不能让前端业务代码直接触碰 Tauri API。

## 2026-06-12：项目记忆进入仓库

决策：新增 `docs/dev-memory/`、`AGENTS.md`、`CLAUDE.md` 和 `scripts/check-dev-memory.mjs`。

理由：

- 项目跨电脑、跨 AI 协作，聊天记录不能作为唯一记忆。
- 需要让每次开工和收工都有固定入口。
- 外部 skill / 插件安装受网络与权限影响，仓库内记忆最稳。

后果：

- 新 AI 进入项目时先读 `AGENTS.md`。
- 每轮开发结束应更新 `DEV_LOG.md` 和 `NEXT_ACTIONS.md`。
- 可用 `node scripts/check-dev-memory.mjs` 检查记忆文件是否完整。

## 2026-06-12：在线资源采用连接器路线

决策：在线层做资源连接器，不做资源站。

理由：

- 合法、低风险、可长期维护。
- 资源丰富度通过书目、版本、来源、官方入口聚合实现。
- 避免项目被版权风险拖死。

后果：

- 官方发行版不内置侵权源。
- 插件系统必须有权限、域名和合规声明。
- v0.5 之前不急于接在线正文。

## 2026-06-12：Calibre 降级为导入来源

决策：Calibre 不作为内部书库底座，只作为导入来源。

理由：

- Calibre 数据模型不适合轻小说的系列、卷、版本、语种、来源关系。
- 自有书库需要服务在线/离线统一模型。

后果：

- 内部使用 `library.sqlite` + 对象仓库。
- Calibre 相关功能应逐步从“书库”改名为“导入来源”。

## 2026-06-12：本地 EPUB / 文件夹导入优先

决策：书库页主路径优先服务用户本地 EPUB 与文件夹导入，Calibre 仅保留为“更多导入来源”里的迁移入口。

理由：

- 轻小说用户不一定使用 Calibre，主路径不能绑定通用电子书管理工具。
- 本项目的核心是自有轻小说书库模型，而不是复刻 Calibre 书库浏览。
- 文件夹导入更贴近日常整理轻小说卷册、系列目录和本地私有资源的使用方式。

后果：

- 书库空状态和主操作优先显示“导入 EPUB / 导入文件夹”。
- Calibre 文案统一改为迁移来源，不再作为默认导入建议。
- 真正的轻小说元数据源、OPDS 和私有来源连接器放到 v0.5+ 继续设计。

## 2026-06-13：schema 实体模型迁移定于 v0.5

决策：`books` 单表在 v0.5 迁移为 `series / volume / edition / asset / source / source_record` 实体模型（合并 work 层进 `volume.kind`，需要时再拆）。

理由：

- 轻小说作品图谱是产品潜力的容器，单表表达不了系列/卷/版本/语种/来源关系。
- 协议 v0.5 冻结 1.0，`LibraryBook` DTO 不能以单表形状冻结。
- 当前 `books.id` 即内容 SHA-256 前缀，annotations / reading_state 以它为键，迁移只动书库层，标注和进度不受影响。

后果：

- v0.5 前 reading-core 需补按 `PRAGMA user_version` 顺序执行的最简迁移框架。
- 连接器（v0.6）不得早于实体模型落地，否则书架 UI 会按来源分裂。
- 资产状态与来源授权拆为正交字段：`asset.availability` × `source_record.rights_status`。

## 2026-06-13：自研边界三层划分

决策：自研「组织层」，复用「原料层」，不碰「无底洞」。

```text
自研：    书库模型、桥接协议、解析缓存管线、插件契约
用现成：  SQLite、FTS5、QuickJS、reqwest、zip/quick-xml/sha2
不碰：    排版渲染引擎、UI 框架、数据库引擎、密码学、CRDT 算法
```

理由：

- 性能与差异化的飞跃全部产生在组织层（数据模型、协议语义、缓存管线）。
- 排版引擎与浏览器几千人年正面竞争，验证成本（排版正确性无自动化 oracle）和长尾维护是 AI 帮不上的部分。
- 「多端用系统 WebView、不自写排版引擎」约束维持不变；如远期要做纵排/ruby 极致排版，作为 v1.0 后的实验性 crate 与 WebView 引擎并存，不推翻主路线。

后果：

- 极致性能路径定为：协议留流式/批量语义 → v0.5 schema 实体化 → 持久化解析缓存 + 并行导入 + 封面缩略图。
- 不换 TypeScript；WASM 仅作为远期纯 Web 壳的编译目标预留，现在零动作。

## 2026-06-13：协议 v0.5 冻结前的演进预留

决策：v0.5 冻结 1.0 之前必须完成三项预留（清单已写入 `docs/resource-library-plan/8_桥接协议_v0.1.md`）。

1. `LibraryBook` DTO 预留实体模型可选字段（`seriesId / volumeId / editionId`），或明确以「新增可选字段」规则演进并先定语义。
2. 「资源通道」升级为协议设计原则：消息面只传 id/路径/元数据，大字节一律走资源通道（URL scheme / 流式），桌面文件夹导入优先走路径版 `library.import`。
3. 评估批量/预取语义（如 `chapter.prefetch`）与结构化错误码。

理由：

- 协议是全系统唯一总线，冻结后只能加不能改，是最不可逆的决策点。
- `importBytes` 整块字节搬运在千本级导入时会成为瓶颈本身。

后果：

- 冻结前的协议改动按本清单逐项核对，未完成不冻结。

## 2026-06-13：v0.3.1 先做便携测试包，不做自动更新器

决策：v0.3.1 的“可下载软件”先采用 Windows 便携测试包 + `LightNovel Reader Launcher.cmd` 启动器；真正自动更新器推迟到发布链路、签名和更新源稳定后再做。

理由：

- 当前唯一发布阻塞仍是 P0 实机冒烟，不能把签名、更新 manifest、差分更新和安全校验塞进 v0.3.1。
- 便携包已经能满足“下载后双击测试”的核心需求。
- 自动更新涉及信任链，不能只做一个看起来像更新器的壳。

后果：

- `npm.cmd run package:beta` 生成 `dist-beta\*.zip`，包内包含 `reader.exe`、启动器、README、VERSION 和冒烟 EPUB 样本。
- v0.3.1 仍需人工 P0 通过后才分发测试包。
- 后续如做自动更新，需单独设计版本源、签名/校验、回滚和发布说明流程。

## 2026-06-13：新增 Web 下载安装器，但不替代自动更新设计

决策：新增 `LightNovelReaderSetup.exe` 生成链路，用作 v0.3.1 下载并安装便携测试包的 Windows bootstrapper。

理由：

- 用户侧更自然的入口是下载一个 `.exe`，由它自动下载、校验和安装阅读器。
- 当前阶段可以通过 SHA-256 校验的 zip 安装器满足测试分发需求。
- 自动更新属于更高风险的发布基础设施，仍需签名、版本 manifest、回滚和更新源设计，不能和 v0.3.1 下载器混为一谈。

后果：

- `npm.cmd run installer:web` 生成 `dist-installer\LightNovelReaderSetup.exe`。
- 公网发布时必须传入 HTTPS zip URL 和 SHA-256；本地路径仅用于测试。
- P0 未通过前，下载器只能作为内部候选，不对外分发。

## 2026-06-13：前端视觉资产只使用原创/自有授权插图

决策：二次元化视觉使用原创生成或自有授权的角色/风景插图，不引用现有动漫、游戏、轻小说 IP 角色。

理由：

- 项目定位是开源免费与长期可分发，不能让前端视觉被版权风险卡住。
- 原创看板娘和风景插图足以建立轻小说风格，不需要借用知名角色。
- 视觉资产应该服务阅读与书库，不应盖过正文或主操作。

后果：

- 当前资产位于 `public/illustrations/`，作为低透明背景和空状态插图使用。
- 后续新增视觉资产必须记录来源，默认走原创/可授权素材。
- 动态效果保持轻量，并提供 `prefers-reduced-motion` 降级。

## 2026-06-13：章节 HTML 必须在 core 侧做安全清洗（防 XSS）

决策：EPUB 章节 HTML 在 `reading-core::html_sanitizer` 内强制安全清洗——移除
`<script>/<iframe>/<object>/<embed>/<applet>` 与 `<base>/<link>/<meta>`、剥除所有 `on*`
事件处理属性、中和 `javascript:/vbscript:/data:text/html` URL。清洗先于排版改写执行。

理由：

- 正文经 `content.innerHTML` 注入**主文档**（非真 iframe），主文档持有 `window.__TAURI__`
  （`withGlobalTauri:true`）且 `tauri.conf.json` 的 `csp:null`。innerHTML 虽不执行 `<script>`，
  但 `onerror/onclick/ontoggle` 等事件处理属性会触发 → 恶意 EPUB 可调 Tauri 命令。
- EPUB 是不可信输入（用户导入任意文件；未来连接器抓远程），属必须防御的攻击面。

后果：

- `html_sanitizer` 新增安全 pass，10 个安全单测覆盖常见向量（脚本块/未闭合脚本/事件属性/
  js: 链接/混淆 js: /iframe/object/embed/meta/裸尖括号文本/CJK 属性保真）。
- **当前实现是基于字符串扫描的“足够好”防线，非 HTML5 解析器级**。后续建议（择期、需评估
  新依赖并记入本文件）：引入 `ammonia`（基于 html5ever 的白名单清洗）做正规化抵御嵌套绕过。
- **纵深防御待办**：收紧 `tauri.conf.json` 的 `csp`（现为 null），但需先确认不破坏
  `reader-img://` 自定义协议与内联样式，且要实机验证——列入 v0.4 安全项，未实机不改。

## 2026-06-13：v0.5 实体模型 schema 草案先行（设计不落地）

决策：在迁移框架就位后，先产出 v0.5 实体模型的建表 + 回填 SQL 草案
（`docs/resource-library-plan/10_书库实体模型_v0.5_schema草案.md`），但**不注册为迁移、不建表**，
避免在产品尚未采纳实体读写前留下死表。

理由：

- 实体读写路径切换是 v0.5 大工程、需实机验证；建空表属过早。
- 草案把 §2 四层模型（合并 work 进 volume.kind）、§3 正交状态字段、§4 建表 SQL、§5 回填策略
  （`asset.id = books.id` 内容哈希不变 → 标注/进度零改动）、§6 DTO 演进、§7 分步迁移顺序固化下来，
  v0.5 可直接粘贴为 library `MIGRATIONS` 的 version 2。

后果：

- v0.5 实施按草案 §7 四步走（双写 → 切读 → UI 系列视图 → DROP books），每步独立可 cargo test。

## 2026-06-16：引入 image 依赖做封面缩略图

决策：reading-core 加 `image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }`，
导入时生成不超过 240×360 的缩略图，书架优先加载缩略图（`loading="lazy"`）。

理由：

- 书架是最先暴露的性能点；几百本一次性加载原图卡顿、占内存。缩略图 + 懒加载是直接收益（DECISIONS 2026-06-13 性能优先级第 1 项）。
- 仅启用 png/jpeg 解码以控编译体积；其它格式（SVG/webp/损坏）解码失败时缩略图 fail-open 跳过，书架回退原图，导入不受影响。

后果：

- `books` 新增 `thumb_path` 列，经迁移框架 **version 2**（`ALTER TABLE`）落地——这是迁移框架上线后的第一个真实增量迁移，验证了「绝不改 v1、只追加新版本」的纪律。
- `LibraryBook` DTO + 协议新增可选 `thumbPath`（符合「只增可选字段」演进规则）。
- v0.5 实体模型迁移顺延为更高版本号（不再是 v2）。

## 2026-06-16：读路径锚定 edition，books 退为只读镜像

决策：v0.5-c 把书库查询（list/search/get）从「锚定 books 表 + LEFT JOIN 实体表」翻为
**锚定 `edition`**（`FROM edition JOIN volume JOIN series LEFT JOIN asset`），不再读 `books`。
「书架的一个条目 = 一个 edition（版本）」：本地条目有 asset（availability=local），
远程 `metadata_only` 条目只有 source_record/无 asset（availability=remote）。
配套：迁移 v4 把 `thumb_path` 从 books 迁到 asset；`LibraryBook.filePath/fileSize` 转可选。

理由：

- 远程元数据条目无本地文件，`books`（`file_path/file_size NOT NULL`）**结构上装不下**它们；
  只要读路径锚在 books，连接器拉来的索引就永远上不了书架。锚定 edition 是元数据连接器能
  显示任何东西的唯一前置。
- 选 edition 作条目单位：文库版/Web 版/译本天然是不同 edition，契合「同一作品多来源」。
- 核心展示字段全部从实体表取（v3 回填 + 导入双写保证与 books 等价），本地书结果不变。
- `filePath/fileSize` 可选符合协议「只增可选字段 / 预留演进」——`library.open` 对无文件条目
  报错，前端据 `availability` 改走外链。

后果：

- `books` 自此为**只读镜像**（仅导入双写 + touch_last_read 写入，读路径不再触碰），v0.6 可 DROP。
- `touch_last_read` 改为同更 asset（读路径读 `asset.last_read_at`）与 books。
- FTS ≥3 字仍经 books_fts（仅本地可搜）；远程条目可搜需将来填 catalog_fts（草案 §8）。
- 条目 `id`：本地 = 内容哈希（asset.id，open/标注/进度同键），远程回退 edition.id。
- 元数据连接器（AniList/Open Library）现在**无前置阻塞**，可直接写 series/edition/source_record
  并即时出现在书架（availability=remote，只展示 + 外链）。

## 2026-06-16：首个元数据连接器（AniList），分层 = 壳传输 / core 解析落库

决策：新增 `crates/reading-core/src/connectors.rs`（含 `anilist` 子模块）实现首个在线元数据
连接器。分层严格拆开：**core 做查询构造 + JSON 解析 + 落库**（纯函数，可单测/可 wasm，无网络），
**壳做 HTTP 传输**（`src-tauri` 加 `reqwest`，命令 `library_search_remote`）。新增桥接消息
`library.searchRemote` 与 `shell.openExternal`；`LibraryBook` 加可选 `remoteUrl`（读路径子查询回填）。

理由：

- 版权红线：连接器**只取元数据**（索引/封面/简介），正文一律跳官方外链。AniList 收录商业
  出版物 → `rights_status=official_purchase`、`availability=remote`，前端只展示 + `openExternal`。
- 选 AniList：轻小说/ACG 元数据全、公开 GraphQL 无需鉴权、有正规条款；不爬商业站正文。
- core 不引网络依赖（保 wasm/可测）；HTTP 属平台 I/O，归壳。解析/落库是业务逻辑，归 core。
  实测：core 用 fixture JSON 全单测，无需联网。
- 落库走 v0.5-c 的实体读路径：连接器写 series/edition/source_record（`ON CONFLICT` upsert，
  `source_id+remote_id` 派生确定性 id → 重复搜索幂等），edition 锚定的读路径自动把它们列上书架。
- 封面以来源 URL 引用、不再托管（封面版权 + 体积），存 `series.cover_path`；前端按 `availability`
  决定是否经 `resource.url` 转换。

后果：

- 用户最初诉求"全网索引 + 三分闸门"端到端打通：在线找书 → 落库 → 书架显示（虚线卡 + "需购买·官方外链"）→ 点击跳官方。
- 远程条目暂不可全文搜（books_fts 不含它们）；catalog_fts 触发器留待后续（草案 §8）。
- `reqwest`（rustls-tls）进 src-tauri 依赖；core 仍零网络。
- 下一步可加更多连接器（Open Library / 青空文库 OPDS）：复用 `connectors::ingest`，各写一个 parser。

## 2026-06-16：第二个连接器（青空文库）选官方 catalog CSV，不用第三方 API

决策：青空文库连接器的数据源用**官方「全作品扩展目录」CSV**（`list_person_all_extended_utf8.zip`，
`www.aozora.gr.jp`），不用社区 REST API（`api.aozorahack.net`）。新增 `connectors::aozora` 子模块
（`parse_catalog_csv`，纯函数可测）+ `csv` 依赖。PR-A 只做 parser；站内阅览（PR-B）另做。

理由：

- 社区 REST API 是非官方、且实测**连不上**（不可靠）。官方 CSV 是权威、稳定、ToS 干净
  （青空《取り扱い規準》明确允许公共版权作品自由再分发）的唯一可靠源。
- **按表头名取列**（作品ID/作品名/作品著作権フラグ/図書カードURL/姓/名），抗列序变化；
  `著作権フラグ=なし` → `public_domain`。这是首个 rights 非 official_purchase 的源 → 未来可站内阅览。
- 加 `csv` crate：扩展目录是带引号 CSV，标准库解析胜过手搓（手搓 CSV 是已知 bug 重灾区）。
- parser 纯函数（&str → Vec<RemoteEntry>，无 I/O，可 wasm/可测），与 anilist 同分层。

后果：

- core 测试 +5（过滤/去重/rights 映射/limit/缺列报错/ingest 上架）。
- **传输层是待决项**：官方 CSV ≈ 13MB（zip 更小）。壳须「下载一次 + 缓存复用 + 解压」，
  不能每次在线找书都拉全量。这步（含缓存策略与首拉 UX）单独评估后再做，故 PR-A 只含 parser。
- PR-B（站内自由阅览）：拉公共版权正文（官方 text URL）→ 导入为本地 asset（availability 转 local/cached）
  → 走现有读路径。需新增 acquire 类协议消息 + 前端「阅读/获取」按钮。

## 2026-06-16：青空公共版权正文获取先合成为单章 EPUB asset

决策：`library.acquireRemote` 当前只支持青空文库 `rights_status=public_domain` 条目。获取流程为：壳侧按需下载/缓存官方 catalog ZIP，读取条目的 `XHTML/HTMLファイルURL`，只允许 `aozora.gr.jp` 官方 URL；下载正文后交给 core 合成为一个单章 EPUB，写入 `library/objects/<assetId>.epub`，挂到既有 `edition`，`asset.availability='cached'`，`source_record.availability` 同步为 `cached`。

理由：

- 公共版权正文可合法站内阅读，但仍需硬门：`著作権あり` / `official_*` / `unknown` 一律拒绝下载，只保留官方外链。
- 合成 EPUB 能复用现有 `library_open`、章节解析、安全清洗、进度和标注链路；不用在 v0.5-e 额外引入「HTML 可读资产」类型和第二套阅读路径。
- 青空官方 XHTML/HTML 自带 ruby，路径比 shift-jis txt + 青空标记解析更轻，适合作为站内自由阅览的第一步。
- 传输层仍归壳（HTTP、ZIP 解压、缓存刷新），解析目录与落库/资产生成归 core；消息面只传 id/元数据，不传大字节。

后果：

- `LibraryBook` 新增可选 `rightsStatus`；前端用它区分「公共版权经典 · 可站内读」与「需购买/官方外链」。
- 新增协议 `library.searchRemoteSource(source, query)` 与 `library.acquireRemote(id)`；旧 `library.searchRemote(query)` 保留为 AniList 兼容入口。
- 青空目录按需下载，缓存 7 天；只在用户主动选择青空文库搜索时触发，不影响只用 AniList/本地书库的用户。
- 若未来要支持多章 HTML、青空 txt 标记或非 EPUB 可读资产，应另开设计，不在本决策里隐式扩展。

## 2026-06-16：青空定位为公共版权经典文学，不作为轻小说主来源

决策：保留青空连接器和 acquire 管线，但产品、UI、文档统一把青空标注为**公共版权经典文学**，不把它描述为轻小说来源。轻小说主线仍是：用户自有 EPUB、本地书库、AniList/Bangumi/なろう 等元数据与官方入口；免费全文站内阅读只接公共版权、开放授权或经用户显式安装并承担 ToS 约束的插件来源。

理由：

- 青空文库主要是日本经典文学/公共版权作品，内容受众与商业轻小说不同。
- 青空的价值是零版权风险地验证“远程条目 → 权利判断 → 正文获取 → 本地 asset → 阅读器/进度/标注复用”管线，而不是提供轻小说内容供给。
- 合法免费的商业轻小说全文几乎不存在；なろう、カクヨム、Royal Road 等更接近轻小说/网文，但正文获取涉及站点 ToS 和抓取边界，应由插件运行时与用户显式选择承接。

后果：

- 前端来源选择和远程卡片文案使用“青空文库（公共版权经典）/ 公共版权经典 · 可站内读”。
- 后续若接 Bangumi/なろう，优先做 metadata/官方入口；正文站内阅读必须另做 ToS 审核或放入 v0.7 插件。
- PR/交接文档不得再把青空写成“轻小说源”，只能写作“公共版权经典库”和“站内阅览管线验证源”。

## 2026-06-16：小説家になろう作为官方 Web 小说元数据源接入，不做正文获取

决策：新增 `narou` 在线来源，使用小説家になろう官方小说 API（`https://api.syosetu.com/novelapi/api/`）做在线找书。内核只解析 `ncode/title/writer/story`，落库为 `rights_status=official_free`、`availability=remote` 的远程条目；点击仍跳 `https://ncode.syosetu.com/<ncode>/` 官方页面。`library.acquireRemote` 不支持なろう。

理由：

- なろう比青空更贴近轻小说/网文发现，但作品仍由作者/平台持有版权；官方 API 适合索引、简介、作者与官方入口，不等于授权客户端下载或缓存正文。
- 继续维持分层纪律：HTTP GET 在壳侧，API JSON 解析与落库映射在 `reading-core::connectors::narou`，core 不引入网络依赖。
- 不新增协议消息，复用 `library.searchRemoteSource(source, query)` 并把 source union 扩展为 `anilist|aozora|narou`。

后果：

- 在线找书现在有三个内置来源：AniList（轻小说/ACG 商业元数据）、なろう（官方 Web 小说元数据）、青空文库（公共版权经典 + 可 acquire）。
- 免费/站内全文的轻小说向来源仍归 v0.7 插件运行时和 ToS 门控，不因接入なろう API 而进入内核正文抓取。

## 2026-06-17：Bangumi 作为社区/目录型书籍元数据源接入，不标记为官方授权入口

决策：新增 `bangumi` 在线来源，使用 Bangumi OpenAPI `POST /v0/search/subjects` 搜索书籍 subject（`type=1`），只读取标题、简介、封面和 subject 外链。落库条目使用 `rights_status=unknown`、`availability=remote`，点击跳 `https://bgm.tv/subject/<id>`；`library.acquireRemote` 不支持 Bangumi。

理由：

- Bangumi 对中文/ACG 轻小说发现很有价值，但它是社区/目录型元数据源，不代表作品正文授权、购买授权或官方阅读入口。
- 为避免误导，`unknown` 远程条目 UI 文案统一收紧为“远程条目 · 外链”，不再写“官方外链”。
- 分层纪律不变：HTTP POST 在 Tauri 壳侧，Bangumi JSON 解析与落库映射在 `reading-core::connectors::bangumi`；core 不引入网络依赖。
- 不新增协议消息，继续复用 `library.searchRemoteSource(source, query)`，source union 扩展为 `anilist|bangumi|aozora|narou`。

后果：

- 在线找书现在有四个内置来源：AniList（轻小说/ACG 商业元数据）、Bangumi（中文/ACG 目录元数据）、なろう（官方 Web 小说元数据）、青空文库（公共版权经典 + 可 acquire）。
- Bangumi 不能成为正文抓取或缓存的依据；正文/章节类来源仍留给 v0.7 插件运行时、ToS 门控和用户显式选择。

## 2026-06-17：目录搜索改由 catalog_fts 覆盖本地与远程 metadata 条目

决策：把 `library.search` 的 ≥3 字搜索从旧 `books_fts` 切到实体目录索引 `catalog_fts`。schema v5 重建 `catalog_fts(edition_id UNINDEXED, title, author, series_title)`，从 `edition → volume → series` 回填，并用触发器同步实体标题、作者和系列变化。

理由：

- v0.5 读路径已经锚定 `edition`，远程 metadata-only 条目没有 `books` 行，继续走 `books_fts` 会让 AniList/Bangumi/なろう/青空远程条目在长词搜索里不可见。
- `catalog_fts` 索引的是目录元数据，不涉及正文，因此符合“远程来源只落 metadata/外链，不抓正文”的版权边界。
- `edition_id` 只用于回连实体读路径，FTS 仍只索引标题、作者、系列标题，避免把来源 URL、rights 状态等非目录字段混入检索。

后果：

- 本地 EPUB 和远程 metadata 条目统一可被 `library.search` 长词命中；短词仍走实体表 LIKE 兜底。
- schema 版本升到 5；旧 v3 预建的空 `catalog_fts` 会被 v5 重建并回填。
- `books_fts` 暂留作旧镜像表的兼容索引，后续 v0.6/v0.7 清理 books 镜像时再统一评估删除。

## 2026-06-17：远程条目与本地书采用人工关联，不做自动合并

决策：新增 `library.linkRemoteToLocal(remoteId, localId)`，用于把一个远程 metadata-only 条目人工关联到一个本地/缓存资产。core 只移动 `source_record.entity_id` 到本地 `edition`，随后删除已经无 asset、无 source_record 的远程空壳；查询层同时隐藏未来重复搜索可能写出的无来源空壳。前端只提供候选列表和显式确认，不做自动推断合并。

理由：

- 远程来源标题、卷号、译名和本地 EPUB 元数据经常不完全一致，自动合并容易误伤。
- 本地 `asset.id` 是内容哈希，也是阅读进度和标注的稳定键；关联来源记录即可把远程 metadata 挂到本地 edition，不需要迁移或重写阅读数据。
- 连接器重搜时可能因为 `source_record` 已移走而重建同一个远程 edition 空壳，查询层隐藏无 asset/无 source_record 条目可避免书架重复长回来。

后果：

- 用户能把 AniList/Bangumi/なろう/青空等远程条目与自己导入的 EPUB 明确绑定。
- 手动关联不会让本地条目获得站内正文授权；`library.acquireRemote` 仍只允许青空公共版权条目。
- 后续若做自动推荐，只能作为"候选排序/提示"，不能直接自动写关联。

## 2026-06-27：Phase 1 WASM 网页端 MVP 架构决策

决策：
1. reading-core 拆 native/wasm features（不是 fork 新 crate），共享同一份 lib.rs 模块树，用 #[cfg(feature)] 切。
2. 浏览器端 EPUB 解析全走 WASM（不引入 JSZip 等外部库），WASM 编译后 500KB。
3. 浏览器端存储用 IndexedDB（元数据/标注/进度）+ OPFS（EPUB 文件本体），OPFS 不可用时降级全 IndexedDB。
4. 平台适配层（platform/index.ts）在非 Tauri 环境直接使用 webBridge，替换旧的 noBridge（全抛错）。
5. zip crate 只用 deflate feature（纯 Rust），避免 bzip2/lzma/zstd 的 C 编译依赖阻断 WASM 构建。

理由：
- feature 拆分（非 fork）避免"两份核心"的维护噩梦。
- WASM EPUB 解析复用已有 Rust 代码（epub_parser + html_sanitizer），不引入 JS 端重复实现。
- OPFS 适合大文件（EPUB 可达几十 MB），IndexedDB 适合结构化小数据。
- deflate-only zip 消除 cc-rs 找 clang 的 WASM 编译阻断问题。

后果：
- WASM 包体积从 74KB（仅分页）增加到 500KB（含完整 EPUB 解析），后续可评估 code splitting。
- 浏览器端不支持插件、OPDS 连接器等需要 shell 能力的功能，需在 UI 层提示。
- webBridge 依赖 WASM 惰性初始化，首次打开书有 ~1s 冷启动延迟（WASM 加载+初始化）。

## 2026-06-27：Phase 2 同步服务 v1 架构决策

决策：
1. 同步服务器为独立 Rust 二进制（crates/sync-server），用户自托管，项目方不运营。
2. 变更日志模型（sync_outbox 追加写）作为同步事实来源，不用全表 diff。
3. 冲突策略分层：阅读进度 LWW + 标注行级 LWW+墓碑复活 + EPUB 内容寻址天然去重。
4. 身份认证用设备配对码（6位数字），不做账号系统 v1。
5. annotations/reading_state sync 列存 storage DB 独立 migration v2，不与 library DB 混。

理由：
- 自托管与 AGPL 协议契合，用户拥有数据。
- sync_outbox O(变更量) 比全量 diff O(全库) 高效，断线 3 天重连只需增量。
- 冲突算法纯函数（无 I/O），wasm feature 下网页端用同一份代码做离线乐观合并。
- 设备配对码降低门槛（不需要邮箱/手机），library_id 预留未来账号系统扩展点。
- storage DB 独立 migration 避免跨库 ALTER TABLE 失败（annotations 表不在 library DB）。

后果：
- sync-server 需要用户自行部署（NAS/VPS），增加运维门槛；缓解措施：桌面端内置零配置局域网模式（Phase 3 实现）。
- sync_outbox 触发器增加写入开销，但单条 INSERT 额外写一条 outbox 行，量级可接受。

## 2026-06-27：Phase 3 桌面端独立化决策

决策：
1. 托盘实现用 Tauri v2 内置 `tray-icon` feature + `TrayIconBuilder`，不引入额外插件。
2. 关闭到托盘：监听 `CloseRequested` 事件 `prevent_close()` 后 `hide()` 窗口。
3. 文件关联通过 `tauri.conf.json` 的 `bundle.fileAssociations` 配置，NSIS 安装器自动注册。
4. 自动更新用 `tauri-plugin-updater`，endpoint 指向 GitHub Releases。
5. 冷启动优化：窗口 `visible: false` + setup 中 200ms 延迟 `show()`，避免白屏闪烁。

理由：
- Tauri v2 内置 tray API 成熟，无需额外插件。
- 关闭到托盘是桌面应用标准行为，实现简单（3 行）。
- 文件关联由 NSIS 安装器自动处理，无需手写注册表。
- updater 插件官方维护，支持 passive 安装模式。
- 启动遮罩策略（先隐藏再延时展示）是最简单的白屏避免方案。

后果：
- 托盘图标需 `.ico` 格式，已用 `default_window_icon()` 取 `icons/icon.ico`。
- `tauri.conf.json` 中 `identifier` 从 `com.tauri-app.reader` 改为 `com.lightnovel.reader`。
- 关闭窗口不再退出程序，用户需通过托盘菜单退出——需在前端 UI 提示。

## 2026-06-27：Phase 4 GPU 翻页 + PWA 决策

决策：
1. GPU 翻页用 CSS transform 双缓冲：两层 `reader-page-layer` 绝对定位叠加，动画只动 `transform: translate3d`（触发合成器加速，不触重排重绘）。
2. 翻页时长 220ms + cubic-bezier(0.2, 0.7, 0.3, 1) 缓动曲线。
3. 快速翻页（动画进行中再次翻页）直接替换弃层内容，不等待 transitionend。
4. PWA 用 vite-plugin-pwa + autoUpdate 模式，precache WASM/JS/CSS/图标共 16 条目。

理由：
- `transform` 只触发 GPU 合成层，不触发 layout/paint，是 Web 端性能最优的动画方式。
- 双缓冲避免白屏或闪烁，动画期间始终有内容展示。
- 快速翻页不排队等待，避免用户狂点翻页时卡死。
- PWA 让网页端可离线使用 + 添加到主屏幕，与"本地优先"定位一致。

## 2026-07-20：插件 HTTP 限速跨 Runtime 共享，official-free 安装前确认源站条款

决策：

1. 桌面插件 HTTP 使用 app-wide 精确域名调度器，同域请求最短间隔固定为 1 秒；一次性 QuickJS Runtime 不各自持有限速状态。
2. `official-free` 且申请 `http` 的 manifest 必须声明 HTTPS `legal.termsUrl`，安装预览展示条款并要求用户显式确认。
3. 继续拒绝 `official-free + acquire` 进入官方仓库；本轮条款门只允许受控的搜索/详情/临时章节读取，不等于授权缓存正文。

理由：

- Runtime 每次调用即销毁，若限速器放在 Runtime 内，连续 `search/getBook/getChapter` 会分别从零计数，无法形成真实每域上限。
- official-free 内容仍受版权和站点规则约束；条款地址、宿主限速和用户确认必须同时存在，不能只靠插件注释自律。
- 收藏来源与缓存正文是不同授权动作；前者只存元数据/外链，后者仍只对公共版权和开放授权设计。

后果：

- `PluginLegal` 新增可选 `termsUrl`，`PluginValidation` 新增 `requiresSourceTermsConfirmation`；均为协议 additive 字段。
- 本地安装继续复用既有 `confirmUserLegal` 参数承载显式法律/条款确认，避免冻结候选协议改名。
- 真实公网来源的站点级更严格频率规则未来可在 1 秒宿主基线上追加，不能放宽到低于宿主默认值。

## 2026-07-21：正式分发必须同时通过两个独立信任域

决策：

1. 正式分发前必须开启官方插件强制验签，并配置至少一枚合法 Ed25519 插件发布公钥。
2. 同一门禁必须确认 Tauri updater 公钥非空；插件包签名公钥不得复用为 updater 公钥。
3. 门禁只挂到正式分发入口，不阻断日常开发构建和离线测试。

理由：

- 当前代码已具备验签能力，但空 keyring、关闭强制开关仍允许人工白名单模式；若打包流程不阻断，容易把预发布配置误当正式安全状态。
- 插件包与应用更新的签名对象、轮换范围和泄露影响不同，共用密钥会放大单点失陷范围。
- 在密钥尚未由维护者安全生成时，代码不能擅自制造或提交“正式私钥”；失败应是清晰、可测试的发布前置条件。

后果：

- `check:release-trust`、`test:release-trust` 成为正式分发前的固定检查。
- `package:beta`、`installer:web` 和 `release:build` 在当前未配置仓库中按设计失败。
- 正式公钥注入、密钥轮换与撤销演练仍是维护者下一项发布工作。

## 2026-07-25：激活首批正式发布信任根

决策：

1. 首批官方插件签名 keyId 固定为 `lnr-plugin-2026-01`，公钥编译进桌面壳，官方索引立即切换为强制签名。
2. Tauri updater 使用独立、带密码的私钥；仓库只保存其公钥，并启用 `createUpdaterArtifacts=true`。
3. 发布门必须同时检查 updater 产物开关，不能只因存在 updater 公钥就把仓库判定为可发布。

理由：

- 公钥与对应私钥已由维护者在仓库外生成，满足从人工白名单模式切换到正式信任根的条件。
- updater 有公钥但不生成签名产物时，构建仍无法形成可用更新链；门禁必须覆盖实际产物配置。
- 两套私钥的生命周期和影响范围不同，继续保持独立存储、独立轮换。

后果：

- unsigned 官方插件索引会被拒绝；首批正式索引必须先签署全部 zip。
- `release:build` 必须由维护者或 CI 从秘密管理注入 updater 私钥路径与密码。
- 私钥、密码、秘密目录路径和临时明文均不得写入仓库、日志或发布附件。

## 2026-07-25：插件仓库与应用更新共用应用 Release，不共用签名域

决策：

1. 插件 zip/`repository.json` 与 Windows updater 安装器/`.sig`/`latest.json` 放入同一个版本化
   GitHub Release；首轮目标为 Tauri 应用版本 `v0.3.1`。
2. 不创建会成为仓库 latest 的插件专用正式 Release，避免
   `releases/latest/download/latest.json` 被不含 updater 清单的 Release 截断。
3. 插件索引必须从包内真实 manifest 生成；签名时强制匹配编译内公钥，上传前再以只读公钥工具独立验收。
4. updater 版本以 `src-tauri/tauri.conf.json` 为权威；不得误用当前不同步的 npm 包版本。

理由：

- GitHub 的 latest Release 同时是当前 updater 静态端点；插件资产单独成为 latest 会让客户端找不到
  `latest.json`。
- 从外部手填 manifest 容易造成索引描述与实际签名包漂移；从 zip 提取可把发布元数据绑定到真实候选。
- 公钥预期值门可以在签名写出前发现选错私钥；独立验收避免只相信同一个签名过程。
- 当前 `package.json` 与 Tauri 配置版本不同，正式桌面产物必须跟随 Tauri 应用版本。

后果：

- `v0.3.1` Release 只有在插件仓库、公钥验收、updater `.sig`、`latest.json` 和安装测试同时就绪后才公开。
- 插件签名与 updater 签名仍使用两套独立私钥；“共用 Release”不代表共用密钥或信任域。

## 2026-07-25：Windows updater 以 NSIS 为唯一主产物，MSI 不阻断更新发布

决策：

1. Windows updater 的 `latest.json` 固定引用 NSIS 安装器及其 `.sig`；交互式签名脚本只构建 NSIS。
2. MSI 保留为可选辅助格式，单独诊断和验收，不参与 updater 发布是否可继续的判定。
3. WiX 语言固定为 `zh-CN`，使中文文件关联元数据使用可编码的代码页。

理由：

- Tauri updater 同一平台只需要一个有效安装器 URL/签名；NSIS 已配置 current-user + passive，符合当前更新路径。
- 首次演练中，MSI 的默认 1252 代码页先触发 `LGHT0311`；修复后本机 Windows Installer 服务又使
  ICE01–ICE09 触发 `LGHT0217`。两者都不应阻断已经可工作的 NSIS。
- NSIS 无签名真实构建已通过，且其 Tauri 官方打包工具已完成下载与哈希校验。

后果：

- `release:build:updater` 成为 Windows updater 的签名构建入口；`release:build` 仍可用于环境完整时同时构建两种格式。
- MSI 发布前需修复本机 Windows Installer 服务并单独复验；不能把 `--no-sign` 产物当作正式 updater。

## 2026-08-04：Gutenberg 以稳定正式身份首次发布

决策：

1. 首次公开前将插件 id 从 `gutenberg-test` 改为稳定的 `gutenberg`，显示名为
   `Project Gutenberg`，资产名为 `gutenberg.zip`。
2. 因旧 id 从未公开发布，正式身份从插件版本 `0.1.0` 开始，不承诺对仓库外测试候选的升级兼容。
3. 统一 `v0.3.1` Release 只允许上传正式 `gutenberg` 资产，不上传任何 `gutenberg-test` 旧候选。

理由：

- 插件 id 是安装、本地存储和后续升级的稳定身份；公开后再改名会被客户端视为两个插件。
- 测试字样会让用户误以为资产不可长期使用；当前已有离线回归、公网 E2E、合规限速与强制验签支撑正式定位。

后果：

- 所有当前文档、运行时夹具、包名和发布索引统一使用 `gutenberg`。
- 旧 `gutenberg-test` 候选仅保留于本机仓库外作审计，不进入 GitHub Release。

## 2026-08-04：updater 资产名必须在生成 latest.json 时对齐 GitHub

决策：

1. Windows updater 公开资产名只使用字母、数字、点、下划线和连字号，不保留 Tauri 默认文件名中的空格。
2. `prepare:updater-release` 默认把安装器名中的空格替换为点号，复制安装器和 `.sig`，
   并用同一最终名生成 `latest.json` URL；`--asset-name` 可显式覆盖，但仍必须通过安全文件名校验。
3. 草稿上传后必须以 GitHub 返回的资产名、大小和 SHA-256 再与本地候选逐项比对。

理由：

- GitHub 实际上传时将 `LightNovel Reader_...exe` 规范化为 `LightNovel.Reader_...exe`，
  而原 `latest.json` 仍指向带空格 URL，若公开将导致自动更新 404。
- 签名覆盖安装器字节而不是资产名，因此重命名不需要重新签名，但 URL 必须重新生成。

后果：

- 增加 `test:prepare-updater-release`，固定“原始名带空格 → 输出名与 URL 使用点号”的回归。
- 首次公开候选升级为 RC5；草稿 Release 只保留 GitHub-safe 的五个资产。

## 2026-08-18：首个公开应用版本统一为 v0.7.0

决策：

1. 不公开既有 `v0.3.1` 草稿候选；首个公开应用版本统一为 `v0.7.0`。
2. npm、Tauri 与三个第一方 Cargo 包使用同一个三段 SemVer，并由版本一致性门禁阻止漂移。
3. 桥接协议 `1.0-rc.1` 与插件版本（首个 Gutenberg 为 `0.1.0`）保持独立，不随应用版本强行改号。
4. 旧 v0.3.1 签名安装器、`.sig` 与 `latest.json` 不得通过改名复用；必须从 v0.7.0 源码重新构建、
   签署并生成更新清单。插件 zip 只有在字节完全不变且复验签名通过时才能复用，`repository.json` 的版本化 URL
   仍需重新生成和验收。

理由：

- 当前已经完成路线中标为 v0.7 的插件来源、签名仓库和发布链能力，公开时仍显示 0.3.1 会误导用户。
- 仓库尚无公开 tag 或 Release，此时统一版本不会破坏既有公开升级承诺。
- npm、Tauri、Cargo、协议和插件版本混在一起会让安装包、User-Agent、发布文档与故障报告持续漂移。

后果：

- 增加 `check:version` / `test:version`，并接入开发检查、构建与所有分发入口。
- 旧 v0.3.1 RC5 和草稿 Release 仅保留为历史证据，不再满足发布条件。
- 正式发布前必须生成新的 v0.7.0 NSIS、updater 签名、`latest.json` 与版本化插件仓库索引，并重新核对远端资产。

## 2026-08-18：统一发布候选采用精确资产白名单与公钥侧验收

决策：

1. 正式上传前把 updater 三资产与插件仓库资产放入单一候选目录，并运行 `verify:release-candidate`。
2. 候选目录只允许 `latest.json`、`repository.json`、清单引用的 NSIS/`.sig` 与插件 zip；任何其他文件都阻断。
3. 验收只读取 Tauri 版本、Release URL 和编译内公钥：核对 updater 签名引用，并用 keyring 独立验证插件包字节；
   不读取、不推导、不缓存任何私钥。
4. 当前单插件首发应恰好是五资产；后续增加插件时，允许集合只随 `repository.json` 的已签名条目扩展。

理由：

- v0.3.1 RC 演练已经暴露文件名/URL 漂移，分散执行多个脚本仍可能把旧 tag 或中间文件带入上传目录。
- “目录里有什么就上传什么”可能误传 unsigned 索引、旧候选甚至密钥材料，精确白名单比后缀黑名单可靠。
- 公钥侧验收可独立于签名过程执行，降低同一脚本既生成又自证造成的盲区。

后果：

- 新增合法五资产、updater 签名漂移、旧 tag、插件篡改和额外密钥文件回归。
- 发布文档要求先通过统一验收，再用输出的大小/SHA-256 与 GitHub 远端资产二次核对。

## 2026-08-20：首次公开前轮换不可解锁的 updater 密钥

决策：

1. 旧 updater 私钥文件保留为历史材料，不覆盖、不提交；因密码不可用，停止把它用于正式候选。
2. 仓库尚无公开 Release 或 Tag，尚未向用户承诺旧 updater 公钥，因此在首次公开前生成新的带密码密钥，
   只把新公钥写入 `tauri.conf.json`。
3. 新密码仅由维护者保存到密码管理器；构建继续使用隐藏交互输入，不写入命令、日志、仓库或发布附件。
4. 以新密钥重新构建完整 NSIS 与 `.sig`，不得复用旧密钥签署的任何 updater 资产。

理由：

- 加密私钥密码无法安全恢复，反复猜测不能成为可重复发布流程。
- 尚无公开安装版本时轮换不会切断用户更新链；公开后再轮换则必须设计旧钥签新版本的迁移窗口。

后果：

- v0.7.0 五资产必须绑定新公钥对应的签名，远端 Release 目标提交也必须包含该公钥。
- 旧 v0.3.1 草稿与旧 updater 资产继续只作历史证据，不满足发布条件。
