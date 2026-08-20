# 开发日志

## 2026-07-20：插件仓库签名发布链离线 smoke

变更：

- Tauri 下载后包校验提取为可单测胶水函数，固定顺序为包大小上限 → SHA-256 → Ed25519；预览和安装仍共同调用
  同一下载/校验入口。新增 RFC 8032 向量测试，覆盖合法签名、哈希先于签名失败、坏签名和强制模式拒绝 unsigned。
- 新增 `scripts/smoke-plugin-repository-signature.mjs` / `smoke:plugin-repository-signature`：每次在系统临时目录生成
  Ed25519 密钥、真实插件 zip 与仓库索引，调用正式 `sign-plugin-repository`，验证原始 zip 签名、单字节篡改和无关公钥拒绝，
  再串联 reading-core 与 Tauri 定向测试。
- smoke 临时目录带专用 marker，默认只在确认目标位于系统临时目录且 marker 存在后清理；临时私钥默认随夹具删除。
  `--keep-data` 仅用于本地诊断，文档明确禁止发布保留目录。
- 补回已有 WebDriver 仓库生命周期脚本的 `smoke:plugin-repo` npm 入口；离线签名 smoke 与真实窗口 smoke 的覆盖边界已在
  SDK、README、发布测试文档和限制说明中区分。
- 同步 PROJECT_MEMORY 与 NEXT_ACTIONS：离线发布链回归已完成，正式密钥/受控 HTTPS 仓库窗口复验仍待外部发布流程。

已验证：

- `cargo test -p reader plugin_repository_command_tests -- --nocapture` 通过：4 passed。
- `node --check scripts/smoke-plugin-repository-signature.mjs` 通过。
- `npm.cmd run smoke:plugin-repository-signature` 通过：签名工具、原始字节验签、单字节篡改、错误公钥、
  reading-core 10 个仓库测试和 Tauri 4 个胶水测试全部通过；临时私钥/夹具已删除。
- `npm.cmd run tauri -- build --debug --no-bundle` 通过，生成包含本轮改动的 `target/debug/reader.exe`。
- `npm.cmd run build` 通过：18 modules，WASM 569.10 KiB，PWA 22 个 precache 条目。
- `cargo test --workspace` 通过：reading-core 149 passed；桌面壳 7 passed / 1 ignored；其余 crate/doc tests 通过。
- `npm.cmd run check:project` 通过：arch / dev-memory / protocol / wasm 四项守卫全绿。
- 限定范围 `rustfmt --check`、两个新增/修改 smoke 脚本语法检查与 `git diff --check` 通过；后者仅有 Windows LF/CRLF 提示。

待验证：

- 仍缺带正式 keyring 的受控 HTTPS 仓库，因此本轮不声称真实 Tauri 网络索引加载、预览/安装二次下载已通过。
- WebDriver 本地包生命周期 smoke 已把包检查/安装失败改为硬失败并实际重跑；旧 149 驱动与 WebView2 150 不匹配，
  从微软官方临时下载精确匹配的 `150.0.4078.83` 后仍在会话创建后遇到 `not connected to DevTools`。
  对照运行既有 `smoke:plugin-source` 发生相同断开，判定为当前 Tauri WebDriver/GUI 环境共性阻断；临时驱动已删除。
- 正式私钥生成、秘密管理、轮换/撤销和强制签名开关需要发布者在独立离线流程中完成。

下一步：

- 获得正式发布密钥授权后，录入公钥/稳定 keyId、签署全部官方包并切换强制签名；随后在真实 HTTPS 环境跑窗口 E2E。
- 独立继续正常公网 DNS 下的 Gutenberg 获取入库复验，以及便携/NSIS 数据保留检查。

## 2026-07-20：官方插件仓库 Ed25519 原始包验签

变更：

- `reading-core::plugin_repository` 新增编译期可信公钥环校验与 Ed25519 验签；签名对象固定为下载到的原始插件 zip 字节，
  不是 JSON、哈希文本或解压目录。签名声明必须使用已知 `keyId`，公钥与签名分别严格解码为 32/64 字节。
- 官方仓库索引加载会校验签名元数据和可信 `keyId`；预览与安装会分别重新下载包，并依次完成 SHA-256 与 Ed25519 验签，
  不复用上一阶段的临时可信结果。非法签名按 `forbidden` 返回。
- 桌面壳新增独立 `plugin_trust` 编译期公钥环；仓库不保存私钥。当前尚无正式发布公钥，因此公钥环为空，
  `REQUIRE_OFFICIAL_PLUGIN_SIGNATURES=false`；未签名条目仅在显式人工白名单模式下允许并显示警告，声明了未知签名的条目仍会被拒绝。
- 桥接协议为仓库包预览/安装增加可选 `signature` 参数；前端候选项区分“待下载验签”和“未签名 · 人工白名单”，
  避免把只经过索引形状校验的条目误报为已验签。
- 新增 `scripts/sign-plugin-repository.mjs` 与 `npm.cmd run sign:plugin-repository`：用离线 PKCS#8 Ed25519 私钥签署原始 zip，
  校验索引中的 SHA-256/大小，输出带签名索引和供公钥环录入的原始公钥 Base64；脚本不写出私钥。
- `reading-core` 原生 feature 新增已有锁文件中的 `ring` / `base64` 可选依赖，WASM feature 不携带这两个依赖。
- 同步仓库 JSON Schema、SDK README、桥接协议、插件契约、开发大纲、决策、工程陷阱、项目记忆与下一步队列。

已验证：

- `cargo test -p reading-core plugin_repository::tests -- --nocapture` 通过：10 passed，覆盖有效签名、篡改包、未知 key、
  未签名人工模式与强制签名模式。
- 临时生成 Ed25519 密钥和仓库夹具，运行签名脚本 smoke 通过；临时产物已删除。
- `cargo check -p reading-core --no-default-features --features wasm` 通过，确认原生验签依赖未进入 WASM 配置。
- `cargo check -p reader` 与 `npx.cmd tsc --noEmit` 通过。
- `npm.cmd run build` 通过：18 modules，WASM 569.10 KiB，PWA 22 个 precache 条目。
- `cargo test --workspace` 通过：reading-core 149 passed；桌面壳 4 passed / 1 ignored；其余 crate/doc tests 通过。
- `npm.cmd run check:project` 通过：arch / dev-memory / protocol / wasm 四项守卫全绿。
- `plugin-sdk/repository.schema.json` JSON 解析、签名脚本语法检查、限定范围 `rustfmt --check` 与 `git diff --check` 通过；
  后者仅有 Windows LF/CRLF 提示。

下一步：

- 在离线环境生成并托管正式 Ed25519 发布私钥，仅把正式公钥和稳定 `keyId` 编入 `plugin_trust`，发布首个已签名官方索引后
  将 `REQUIRE_OFFICIAL_PLUGIN_SIGNATURES` 切为 `true`。
- 为签名仓库补真实 Tauri HTTPS 窗口 smoke，覆盖合法签名安装、zip 篡改、未知 key 与预览后安装重新验签。
- 继续在正常公网 DNS 的 Windows/Tauri 环境复验 Gutenberg 获取入库闭环；不为测试放宽 SSRF 门。

## 2026-07-20：开放资源插件 source.acquire 获取入库闭环

变更：

- QuickJS 运行时开放 SDK 可选 `acquire(remoteId, mode)`，对返回的 `AcquireProposal` 做 DTO、长度、
  manifest 精确域名与 core 授权裁决；新增越域 URL 和 rights escalation 拒绝测试。
- 以 additive `source.acquire` / `acquirePluginSourceBook` 接入正式获取：重新执行 `getBook + acquire(cacheForReading)`，
  只接受 manifest/提案一致的 `public-domain/open-license` 和 `application/epub+zip`。
- Tauri 复用 app-wide 每域限速、SSRF/DNS 固定、禁止重定向的插件 HTTP 执行器下载 EPUB；非 2xx 返回结构化
  `httpStatus`，无效 EPUB 返回 `parseError`。下载字节不经过前端 bridge，验证后由 core 写入对象仓库并挂到远程 edition。
- 插件来源搜索卡片和书籍详情对合法 `acquire` 来源显示“获取并阅读”，获取后复用当前内置/外部阅读偏好打开。
- Gutenberg 示例新增 `acquire` capability 与 EPUB3 下载提案，并重新打包联网测试 zip；ignored E2E 扩展为
  `search → getBook → getChapter → acquire proposal`。
- 同步 README、SDK README、协议文档、插件契约、开发大纲、决策、工程陷阱、项目记忆与下一步队列。

已验证：

- `cargo check -p reader` 通过。
- `cargo test -p reading-core --features quickjs runs_async_sdk_flow_with_http_html_kv_and_sanitizing -- --nocapture` 通过。
- `cargo test -p reading-core --features quickjs acquire_rejects_out_of_domain_and_rights_escalation -- --nocapture` 通过。
- `npx.cmd tsc --noEmit` 通过。
- `npm.cmd run build` 通过：18 modules，WASM 569.10 KiB，PWA 22 个 precache 条目。
- `cargo test --workspace` 通过：reading-core 147 passed；桌面壳 4 passed / 1 ignored；其余 crate/doc tests 通过。
- `npm.cmd run check:project` 通过：arch / dev-memory / protocol / wasm 四项守卫全绿。
- `git diff --check` 通过（仅 Windows LF/CRLF 提示）。

环境限制：

- 本轮在沙箱内和经批准的沙箱外均实际运行 Gutenberg 联网 E2E，但系统 DNS 都把
  `www.gutenberg.org` 解析到 `198.18.0.15`；SSRF 门按设计拒绝，因此没有完成真实公网 EPUB 下载/入库窗口复验。
  未为测试放宽安全策略。

下一步：

- 在正常公网 DNS 的 Windows/Tauri 环境安装 Gutenberg 示例，完成搜索、章节预览、“获取并阅读”、对象仓库落盘和重启回读。
- 然后处理官方仓库签名验签/人工白名单发布决策，以及便携包和 NSIS 数据保留复验。

## 2026-07-20：真实 Tauri 来源 smoke、每域限速与源站条款门控

变更：

- 新增 `scripts/tauri-plugin-source-smoke.mjs` / `smoke:plugin-source`，在隔离 app data 的真实
  `reader.exe` 中覆盖插件安装、来源下拉、搜索、详情、纯文本章节预览、显式收藏、来源记录、停用与重启持久化。
- smoke 发现并修复“搜索框本地防抖刷新覆盖在线/插件结果”的 UI 竞态；在线搜索入口现在会先取消待执行的本地刷新。
- 插件 HTTP 执行器改为 AppState 共享实例，所有一次性 QuickJS Runtime 共用精确域名调度器；同域请求最短间隔 1 秒，
  不同域互不阻塞，并统一使用宿主 User-Agent。SSRF/DNS 固定/禁止重定向等既有边界不变。
- manifest `legal` 新增可选 `termsUrl`；`official-free + HTTP` 必须声明有效 HTTPS 条款地址，并在安装前要求用户勾选确认。
  安装预览会显示和打开条款；`PluginValidation` 新增 `requiresSourceTermsConfirmation`。
- 离线测试插件补条款地址并重新打包；桌面 smoke 会显式确认条款。

已验证：

- `npm.cmd run tauri -- build --debug --no-bundle` 通过并生成最新 `target/debug/reader.exe`。
- `npm.cmd run smoke:plugin-source` 在真实 Tauri 窗口通过完整离线来源闭环；同时验证搜索不落库、收藏不获取正文。
- `cargo test -p reading-core` 通过（143 passed）；`cargo test -p reader plugin_executor::tests` 通过（3 passed / 1 ignored）。
- `npx.cmd tsc --noEmit` 与新增 smoke 脚本语法检查通过。

环境限制：

- Gutenberg ignored E2E 已再次实际运行，但当前环境仍把 `www.gutenberg.org` 解析到保留地址，按 SSRF 策略预期拒绝；
  未放宽安全边界，仍需正常公网 DNS 的 Windows/Tauri 环境完成最终复验。

下一步：

- 为 `public_domain/open_license` 设计插件 acquire 提案到本地 asset 的合法获取闭环；
  `official_free/user_declared` 继续只允许外链/临时预览，不因收藏自动缓存。

## 2026-07-20：干净检出 WASM 构建修复与正式插件来源流程

变更：

- 安装并验证 `wasm32-unknown-unknown` 与锁文件匹配的 `wasm-bindgen-cli 0.2.122`，重新生成并跟踪
  `src/worker/reading-core-wasm/` 下的 JS、类型声明与约 569 KiB WASM 二进制。
- 新增跨平台 `scripts/build-reading-core-wasm.mjs`：从 `Cargo.lock` 读取绑定版本，检查 Rust target/CLI，
  编译 `reading-core --no-default-features --features wasm` 后生成浏览器绑定；新增 `check-wasm-artifacts.mjs` 验证文件、WASM 文件头和必要导出。
- `package.json` 新增 `build:wasm` / `check:wasm`，并把产物守卫接入 `check:project` 与生产构建。
- 新增正式 `source.list/search/getBook/getChapter/collect` 桥接，不改变 `plugin.testFlow` 的诊断语义。
  Tauri 统一装载已启用插件并在阻塞线程运行 QuickJS；`source.collect` 会重新执行 `getBook` 后再进入 core。
- 新增 `reading-core::plugin_source`：把用户显式收藏的插件书籍幂等映射为
  `source(kind=plugin) + series/volume/edition + source_record`。稳定键由插件 id + 规范书籍 URL 哈希派生；
  搜索不自动落库，收藏不自动下载/缓存正文，卸载插件也不删除已有来源记录。
- QuickJS 返回值校验收紧：书籍/章节/封面 URL 必须属于 manifest 精确域名；单页最多 200 个搜索结果、
  一本最多 20,000 章，并限制 URL、标题与简介长度。章节 HTML 仍先经 core 清洗。
- 书库在线来源下拉框会动态加入启用插件，支持分页搜索、详情、章节列表、纯文本正文预览、源站打开和显式收藏。
  章节 UI 不把插件 HTML 直接注入主文档，也不加载插件返回的远程资源。
- 修正离线 `scripts/test-plugin` manifest 的 `apiVersion`/capability，重新打包 zip，并新增正式
  `search/getBook/getChapter` 确定性测试与收藏/幂等/授权映射/越界 URL 测试。
- 同步 README、AGENTS、桥接协议、插件契约、开发大纲、决策、工程陷阱、项目记忆和下一步队列。

已验证：

- `npm.cmd run build:wasm` 通过：`reading-core` WASM release 编译与 wasm-bindgen 生成成功。
- `npm.cmd run check:wasm` 通过。
- `cargo check -p reader` 通过，Tauri 已真实编译新增 source commands 与 QuickJS feature。
- `cargo test --workspace` 通过：`reading-core` 145 passed；桌面壳 3 passed / 1 ignored；其余 crate/doc tests 通过。
- `npx.cmd tsc --noEmit` 通过。
- `npm.cmd run build` 通过：18 modules，WASM 569.10 KiB，PWA 22 个 precache 条目。
- `npm.cmd run check:project` 通过：check-arch / check-dev-memory / check-protocol-freeze / check-wasm-artifacts 全绿。
- `git diff --check` 通过（仅 Windows LF/CRLF 提示）。

未完成的验证：

- 尚未在真实 Tauri 窗口安装 `scripts/test-plugin/test-plugin-hello.zip`，人工走完来源下拉搜索、章节预览、收藏、停用与重启流程。
- Gutenberg 公网 E2E 仍受当前环境 fake-IP DNS 阻断；需在正常公网 DNS 环境复验，不能为测试放宽内网防护。

下一步：

- 先跑真实 Tauri 离线正式来源 smoke，再跑正常公网 Gutenberg 流程。
- 补每域限速和来源 ToS/用户确认门控，再考虑任何 `official-free + acquire`。
- acquire 门完成后，只为 `public_domain/open_license` 设计插件正文缓存为本地 asset 的新增流程。

## 2026-07-20：v0.7 QuickJS 契约对齐、沙箱收口与完整试跑

变更：

- 开工时执行 `git fetch --all --prune`，确认本地 `main` 与 `origin/main` 同为 `a03103c`，GitHub 无新提交。
- Tauri 对 `reading-core` 启用 `quickjs` feature，将先前未被桌面构建覆盖的 runtime 路径纳入真实编译。
- 重整 `plugin_runtime`：支持 SDK `export default`、Promise 形状、`search(query, page)` /
  `getBook(bookUrl)` / `getChapter(chapterUrl)` 标量参数、`HttpResponse.text()`、`host.html`、持久化 KV、
  字符集解码、可读 JS 异常堆栈、DTO 校验和章节 HTML 安全清洗。
- `PluginHttpExecutor` 改为返回状态/响应头/原始字节，Tauri reqwest 实现按计划头和超时执行；
  禁止自动重定向，拒绝本机/内网/保留地址，并用已校验的 DNS 结果固定连接，防止重定向和 DNS rebinding 绕过域名门。
- 新增 64 MiB QuickJS 堆、25 秒截止时间、8 MiB HTTP/HTML/JSON 上限和 4 KiB 日志上限；HTTP 超时会受当次 Runtime 剩余时间约束。
- 桥接层新增 `PluginTestFlowResult` 和 `testPluginFlow`；Tauri command 改为 `spawn_blocking` 中自动跑完三个必选方法，
  插件面板显示完整结果。Web 壳显式返回不支持。
- 测试插件和 Gutenberg 示例均改为正式 SDK 写法并重新打包；Gutenberg 阅读链接同时兼容当前 `/cache/epub/...html` 和旧 `/files/...html` 形状。
- 同步 SDK README、插件契约/运行时方案、桥接协议、项目记忆、开发大纲、决策日志和下一步队列。

已验证：

- `cargo check -p reading-core --features quickjs` 通过。
- `cargo check -p reader` 通过。
- `cargo test -p reading-core --features quickjs` 通过（138 passed），含确定性 `search → getBook → getChapter` 夹具。
- `cargo test --workspace` 通过：`reading-core` 138 passed，桌面壳 3 passed / 1 ignored，其余 crate/doc tests 通过。
- `npm.cmd run check:project` 通过：check-arch / check-dev-memory / check-protocol-freeze 全绿。
- 临时补上缺失 WASM 模块的 `.d.ts` 后，`npx.cmd tsc --noEmit` 通过；临时声明已删除，本轮 TS 协议/桥接改动无类型错误。
- `git diff --check` 通过（仅 Windows LF/CRLF 提示）。
- Gutenberg ignored 联网 E2E 已实际运行，当前 Codex DNS 将 `www.gutenberg.org` 映射为保留网段 `198.18.0.15`，
  因新的内网防护被预期拒绝；未为通过环境特例而放宽沙箱。

未完成的验证：

- `npm.cmd run build` 的 check-arch/check-protocol 已通过，但 TypeScript 在主分支既有的
  `src/worker/reading-core-wasm/reading_core.js` 缺失处停止。当前检出同时缺 wasm32 target 与 wasm-pack/wasm-bindgen CLI；
  本轮未联网安装全局构建工具，也未伪造 WASM 产物。

下一步：

- 在正常公网 DNS 的真实 Tauri 窗口安装 `plugin-sdk/examples/gutenberg-test/gutenberg-test.zip` 并点“测试”复验。
- 先补齐可重复的 reading-core WASM 构建/生成流程，恢复干净检出的 `npm.cmd run build`。
- 新增正式 `source.*` 消息/UI，把插件搜索结果转成可收藏的来源记录；不改 `plugin.testFlow` 的诊断语义。
- 在任何 `official-free + acquire` 放行前补每域限速和源站 ToS 门控。

## 2026-06-23：v0.7 官方插件仓库下载校验安装链路

变更：

- 书库“源插件（v0.7 预览）”面板新增官方插件仓库索引 URL 输入、加载按钮与候选插件列表。
- 新增 `plugin.repository.load` / `plugin.repository.inspectPackage` / `plugin.repository.installPackage` 桥接能力；前端仍只通过 `src/platform`，Tauri command 负责 HTTPS 下载与参数搬运，校验/预览/安装仍在 `reading-core`。
- 官方索引加载会先走 `reading-core::plugin_repository` 校验；候选插件逐条下载 zip 后核对 `packageSha256`，再复用 `plugin_package` / `plugin_store` 生成安装预览。
- 安装官方仓库插件时会重新下载 zip 并再次核对 SHA-256，不信任预览阶段临时结果。
- 官方仓库包仍拒绝 `user-declared` 与 `official-free + acquire`，直到 ToS/限速/用户确认门控补齐；当前仍不执行插件 JS，不引入 QuickJS。
- 新增 `scripts/new-smoke-plugin-repository.mjs` 与 `npm.cmd run smoke:plugin-repository-fixtures`，可生成合法插件 zip、SHA-256 与 `repository.json`，为后续真实 Tauri 窗口官方仓库 smoke 提供稳定夹具。
- 夹具生成器只清理自己生成的 `package/`、zip 与 `repository.json`，不递归删除整个 `--out-dir`，避免参数误传造成目录级破坏。
- 修复官方仓库候选“源码”按钮的错误展示：外链打开失败时回显到插件面板。
- 加载新的官方仓库索引前会清空旧安装预览，避免用户在新索引上下文中误安装上一轮校验过的包。
- Tauri 官方仓库包下载命令会在联网前先校验 `packageSha256` 必须是 64 位 hex，并补单测覆盖。
- 同步桥接协议文档、DECISIONS、PROJECT_MEMORY、NEXT_ACTIONS。

已验证：

- `npm.cmd run smoke:plugin-repository-fixtures -- --out-dir .\tmp-plugin-repository-smoke --base-url https://plugins.example.invalid/smoke` 通过；测试产物已删除。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-protocol-freeze.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 123 passed）。
- `git diff --check` 通过（仅 Windows 换行提示）。
- 本轮开始时因 main 新增 `tauri-plugin-dialog` 依赖，先运行 `npm.cmd install` 与联网 Cargo 拉取依赖；依赖拉取完成后 workspace 测试通过。

下一步：

- 给官方索引安装流补真实 Tauri 窗口 smoke：复用插件仓库夹具，接入测试 HTTPS server 或可信 HTTPS fixture URL，验证索引加载、包校验、安装确认、已安装列表刷新。
- 真正做签名验签前先设计 keyring/验签策略；当前 signature 字段仍只是元数据预留。

## 2026-06-23：v0.7 官方插件仓库索引骨架

变更：

- 新增 `reading-core::plugin_repository`，定义官方插件仓库索引 DTO、索引校验与插件 zip SHA-256 校验。
- 官方索引校验覆盖：`schemaVersion=0.1`、最多 500 条、manifest 复用 `plugin_manifest` 策略、拒绝重复插件 id、拒绝 `user-declared`。
- 包校验覆盖：`packageUrl/sourceUrl` 必须 HTTPS，`packageSha256` 必须 64 hex，`packageSize` 必须在 1..=50 MiB。
- 合规边界：`official-free + acquire` 在 ToS/限速/用户确认门控落地前不得进入官方索引；签名字段只校验 `ed25519/keyId/value` 形状并返回 warning，不做密码学验签。
- 新增 `plugin-sdk/repository.schema.json`，同步 README、插件契约、DECISIONS、PROJECT_MEMORY、NEXT_ACTIONS。
- 当前仍不下载、不安装、不执行插件 JS，不新增桥接协议。

已验证：

- `node -e "JSON.parse(...plugin-sdk/repository.schema.json...)"` 通过。
- `cargo test -p reading-core plugin_repository -- --nocapture` 通过（8 passed）。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-protocol-freeze.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 123 passed）。
- `git diff --check` 通过（仅 Windows 换行提示）。

下一步：

- 后续可做官方索引 UI/下载校验链路：索引校验 → 壳下载 zip → SHA-256 校验 → 安装预览/用户确认。

## 2026-06-22：v0.7 插件 host API 策略层

变更：

- 新增 `reading-core::plugin_host`，定义源插件方法、搜索结果、书籍详情、章节正文、`host.http`、`host.kv` 与 `acquire` 的 Rust DTO。
- 新增运行前策略门：停用插件不得运行；`browse/resolveUrl/fetchMetadata/acquire` 必须声明对应 capability。
- 新增 `host.http` 请求计划校验：必须有 `http` 权限、URL 精确命中 manifest 域名、超时限制为 1..=60000ms、忽略 User-Agent/Referer/Cookie/Authorization/Host/Origin 等保留头。
- 新增 `host.kv` 权限与尺寸门控：必须有 `kv` 权限，key 最大 128 字符，value 最大 64 KiB。
- 新增 `acquire` 宿主裁决：`metadataOnly` 不下载；`download/cacheForReading` 第一版只放行公共版权与开放授权，`official_free` 在 ToS/限速门控落地前仍只做 metadata + 官方外链。
- 同步插件 SDK 注释、插件契约、决策日志、项目记忆与下一步队列；当前仍不执行插件 JS、不新增桥接协议。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-protocol-freeze.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 115 passed）。
- `git diff --check` 通过（仅 Windows 换行提示）。

下一步：

- 后续接 QuickJS/JavaScriptCore host 时必须复用 `plugin_host` 策略函数；不要绕过到平台壳直接发 HTTP、写 KV 或缓存正文。

## 2026-06-22：v0.7 插件卸载与覆盖安装收口

变更：

- `reading-core::plugin_store` 新增 `uninstall_plugin`，按安全插件 id 删除 app data 下对应插件目录。
- 重新安装同 id 插件时先删除旧目录再写入新包，避免旧入口文件残留。
- 新增 `plugin.uninstall` 桥接能力与 Tauri command；书库源插件面板增加“卸载”按钮和确认提示。
- 卸载只删除插件文件，不影响书库数据；当前仍不执行插件 JS。
- 同步 README、桥接协议文档、插件契约文档与项目记忆。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-protocol-freeze.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 106 passed）。
- `git diff --check` 通过（仅 Windows 换行提示）。

下一步：

- 后续 v0.7 继续做 host API 纯 Rust 输入输出结构；运行时落地时必须跳过停用插件，且不得加载已卸载插件。

## 2026-06-22：v0.7 插件启用状态骨架

变更：

- `InstalledPlugin` 新增 `enabled` 字段；旧安装记录缺字段时默认视为启用。
- 新增 `reading-core::plugin_store::set_installed_plugin_enabled`，按安全插件 id 更新 `install.json`，并拒绝路径穿越式 id。
- 新增 `plugin.setEnabled` 桥接能力与 Tauri command；书库源插件面板可对已安装插件执行启用/停用。
- `plugin-sdk/examples/*.zip` 加入 `.gitignore`，避免测试打包产物污染工作区。
- 同步桥接协议文档与插件契约文档。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-protocol-freeze.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 104 passed）。
- `git diff --check` 通过（仅 Windows 换行提示）。

下一步：

- 后续 v0.7 可继续做 host API 纯 Rust 输入输出结构；运行时落地时必须跳过 `enabled=false` 的插件。

## 2026-06-22：v0.7 插件安装 UI 与本地存储骨架

变更：

- 新增 `reading-core::plugin_store`，提供插件包预览、安装写入与已安装插件列表读取。
- 插件安装写入 app data `plugins/sources/<plugin-id>/`，保存 `manifest.json`、入口 JS 与 `install.json`；当前仍不执行插件代码。
- `user-declared` 插件必须传入用户显式确认，否则 core 拒绝安装。
- 新增 `plugin.selectPackagePath` / `plugin.inspectPackage` / `plugin.installPackage` / `plugin.listInstalled` 协议能力，桌面壳用 Tauri dialog 原生文件选择器取得 zip 路径，消息面不传 zip 字节。
- 书库新增“源插件（v0.7 预览）”面板，展示插件名称、版本、域名、权限、能力、授权状态、warning 与已安装列表。
- 同步 README、插件契约文档、桥接协议文档与项目记忆。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-protocol-freeze.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 102 passed）。
- `npm.cmd run tauri -- build --debug --no-bundle` 通过。
- `git diff --check` 通过（仅 Windows 换行提示）。

下一步：

- 后续 v0.7 可继续做 host API 纯 Rust 输入输出结构与插件启用/禁用状态；仍不执行第三方 JS、不接正文抓取源。

## 2026-06-22：v0.7 插件 zip 安装包读取骨架落入 reading-core

变更：

- 新增 `crates/reading-core/src/plugin_package.rs`，在 core 侧读取插件 zip 包并复用 `plugin_manifest` 策略。
- `load_plugin_package_zip` 支持根目录包与单层目录包，读取唯一 `manifest.json`、校验 manifest、确认入口 JS 存在并返回入口文本。
- 安全约束：拒绝空包、非 zip、多 manifest、缺入口、入口为空、路径穿越、绝对路径、目录入口、非 UTF-8 文本。
- 单测覆盖 root/nested 包、缺入口、多 manifest、路径穿越、user-declared flags、非 zip。
- `plugin-sdk/README.md` 与文档 9 同步插件 zip 包格式与“安装前不执行插件代码”的规则。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-protocol-freeze.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 99 passed）。
- `git diff --check` 通过（仅 Windows 换行提示）。

下一步：

- 后续 v0.7 可继续做插件安装 UI/权限确认，或补 host API 纯 Rust 接口；仍不接正文抓取源。

## 2026-06-22：v0.7 插件 manifest 策略骨架落入 reading-core

变更：

- 新增 `crates/reading-core/src/plugin_manifest.rs`，作为 v0.7 插件运行时的第一块宿主侧策略骨架。
- `PluginManifest` 强类型覆盖 `apiVersion/id/name/version/entry/domains/permissions/capabilities/legal`。
- `validate_manifest` 强制校验 API 版本、插件 id、semver、入口文件名、域名白名单、权限/能力去重、字段长度和 `user-declared` 明示确认要求。
- `is_url_allowed_by_manifest` 提供宿主侧精确域名白名单检查；只接受 `http(s)`，不做子域通配。
- `official-free + acquire` 会返回 ToS/限速门控 warning；`user-declared` 不具备官方仓库资格，并要求安装时明示确认。
- `plugin-sdk/manifest.schema.json` 同步新增 `capabilities` 与 domains 去重；`source-plugin.d.ts` 同步可选 capability 方法与 acquire proposal 类型；README 补充插件边界。
- `docs/resource-library-plan/9_插件契约_v0.1.md`、`PROJECT_MEMORY.md`、`NEXT_ACTIONS.md`、`DECISIONS.md`、`DEVELOPMENT_OUTLINE.md`、`README.md` 同步当前状态。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-protocol-freeze.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 92 passed）。
- `node -e "JSON.parse(...)"` 校验 `plugin-sdk/manifest.schema.json` 与示例 manifest 均为合法 JSON。
- `git diff --check` 通过；仅有 Windows 换行提示。
- `cargo fmt --check` 未作为本轮门槛：当前仓库既有多处 Rust 文件会被 rustfmt 改写，已仅对本轮新增的 `plugin_manifest.rs` 做局部 `rustfmt`，避免无关格式噪声。

下一步：

- 后续 v0.7 可在此基础上做插件安装包读取/展示权限，再进入 QuickJS host API，不要先接正文抓取源。

## 2026-06-22：补协议冻结自动守门脚本

变更：

- 新增 `scripts/check-protocol-freeze.mjs`，自动核对 `src/platform/protocol.ts`、`src-tauri/src/lib.rs` 与 `docs/resource-library-plan/8_桥接协议_v0.1.md` 的协议冻结关键事实。
- 检查内容包括：
  - `PROTOCOL_VERSION` 是否被协议文档同步记录。
  - TS `BridgeErrorCode` 的全部错误码是否进入协议文档。
  - Rust `BridgeError` 构造器使用的错误码是否全部存在于 TS `BridgeErrorCode`。
  - 协议文档是否保留“新增消息/新增可选字段、不允许改名/删字段/改语义”的冻结规则。
- `package.json` 新增 `check:protocol`，并把它接入 `check:project` 与 `npm.cmd run build` 的前置检查。
- `docs/resource-library-plan/8_桥接协议_v0.1.md`、`PROJECT_MEMORY.md`、`NEXT_ACTIONS.md` 同步记录这条冻结守门纪律。

已验证：

- `node scripts/check-protocol-freeze.mjs` 通过。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run check:project` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过；仅有 Windows 换行提示。

下一步：

- 若验证通过，提交并推送 `codex/protocol-freeze-guard`，开 PR 后再合入主线。

## 2026-06-22：桥接协议推进到 1.0-rc.1 冻结候选

变更：

- 合并 PR #33（`codex/protocol-freeze-audit`）到 `main`，把预取语义与资源通道审计结论带入主线。
- 新建 `codex/protocol-error-freeze-audit`，继续协议冻结前最后一轮错误面收口。
- `src/platform/protocol.ts`：`PROTOCOL_VERSION` 从 `0.1` 推进到 `1.0-rc.1`；`BridgeErrorCode` 新增 `platformError`；当前结构化错误覆盖范围记录为 `book/chapter/library/annotation/reading/opds/shell`。
- `src/platform/tauri.ts`：为 `shell.openExternal` / `shell.openPathExternal` 增加壳侧包装，空 URL/path 返回 `invalidArgument`，系统浏览器或外部阅读器打开失败返回 `platformError`。
- `src/platform/index.ts`：无原生 bridge 的浏览器兜底改为抛 `BridgeError` 形态的 `platformError`，避免协议面继续出现裸 `Error`。
- `docs/resource-library-plan/8_桥接协议_v0.1.md`：标题同步为 v1.0-rc.1，记录文件名沿用原因、shell 错误语义、`platformError` 与冻结候选规则；冻结前检查清单第 3 项标记完成。
- `docs/dev-memory/DECISIONS.md` / `NEXT_ACTIONS.md` / `PROJECT_MEMORY.md` / `AGENTS.md` / `docs/README.md` 同步当前协议状态与下一步交接。

已验证：

- `npm.cmd run build` 通过。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过；仅有 Windows 换行提示。

下一步：

- 提交并推送 `codex/protocol-error-freeze-audit`，打开 PR。
- PR 合并后进入协议冻结候选 review；功能线继续评估 v0.7 插件运行时 / ToS 门控，分发线继续目标机器便携包抽检与 NSIS 卸载保留数据验证。

## 2026-06-22：协议冻结审计收口预取与资源通道

变更：

- 新建 `codex/protocol-freeze-audit`，继续 NEXT_ACTIONS 的协议冻结审计。
- 审计 `src/reader-core.ts`、`src-tauri/src/lib.rs`、`crates/reading-core/src/parse_cache.rs` 与
  `docs/resource-library-plan/8_桥接协议_v0.1.md` 后确认：
  - `ReaderCore.preloadAroundChapter` 已通过 `chapter.get` 有界预取前一章、后一章、后两章。
  - 前端有 `chapterInflight` 去重和 `maxCachedChapters=10` 内存上限。
  - core/Tauri 侧有当前书章节内存缓存与持久化 parse cache。
  - 书内图片走 `reader-img` URL scheme，封面/缩略图走 `resource.url` 或来源 http(s) URL。
- `docs/resource-library-plan/8_桥接协议_v0.1.md`：
  - 新增“章节预取语义（冻结前审计结论）”小节，明确冻结前不新增 `chapter.prefetch` /
    `chapter.getBatch`。
  - 新增“资源通道审计（冻结前审计结论）”小节，明确只保留 `book.open(data)` 与
    `library.importBytes(data)` 两个大字节兜底例外。
  - 冻结前检查清单第 2 项和第 4 项标记完成。
- `docs/dev-memory/DECISIONS.md` 新增决策：协议冻结前不新增章节预取消息，资源通道边界通过审计。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `git diff --check` 通过；仅有 Windows 换行提示。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。

下一步：

- 提交并推送 `codex/protocol-freeze-audit`，开 PR。
- 后续继续协议冻结：结构化错误码范围最终核对、确认是否将 `PROTOCOL_VERSION` 从 `0.1` 进入冻结候选。

## 2026-06-22：合并 PR #32 并重跑便携包候选验证

变更：

- 合并 PR #32（`codex/remaining-bridge-errors`）到 `main`，远端分支已删除；本地 `main` 快进到
  `f74ef4d`。
- 重新运行 `npm.cmd run package:beta`，生成 release 便携候选：
  `dist-beta/lightnovel-reader-v0.1.0-release-windows-x64.zip`。
- 将 zip 解压到 `dist-beta/extract-check-release`，确认包内包含：
  `reader.exe`、`LightNovel Reader Launcher.cmd`、`README.txt`、`VERSION.txt` 与 `samples/`。
- 使用解压后的 release `reader.exe` 跑真实 Tauri 启动冒烟，验证初始 UI、插图加载、书库入口、
  导入按钮、Calibre 迁移入口位置与关闭书库面板。

已验证：

- `npm.cmd run package:beta` 通过。
- `npm.cmd run smoke:tauri -- --tauri-driver C:\Users\41267\.cargo\bin\tauri-driver.exe --native-driver C:\Users\41267\AppData\Local\lightnovel-reader-tools\msedgedriver\149.0.4022.69\msedgedriver.exe --application E:\workspace\game-cooperative-plan\lightnovel-reader\dist-beta\extract-check-release\reader.exe` 通过。

未验证：

- 本轮未重跑 NSIS 安装 / 卸载。
- 本轮未重跑 `smoke:p0` / `smoke:p1` / `smoke:remote-link` / `smoke:opds`。

下一步：

- 若准备发便携测试包，可把当前 zip 作为候选，再按目标机器做下载/解压/启动抽检。
- 若继续开发，优先做协议冻结审计：批量/预取语义、资源通道核对、后续新增命令默认使用
  `BridgeError`。

## 2026-06-22：迁移剩余阅读/标注命令到 BridgeError

变更：

- 从 GitHub 同步最新 `main`：`git fetch --all --prune` 后确认远端 `codex/source-record-panel` 已删除，
  `git pull --ff-only` 快进到 `0b027bf`，拉下 OPDS acquisition URL 持久化等主线更新。
- 新建 `codex/remaining-bridge-errors`，按 NEXT_ACTIONS 顶部优先级继续协议内功。
- `src-tauri/src/lib.rs`：
  - `load_book_from_data` 改为返回 `BridgeError`，解析失败走 `parseError`，锁/状态写入失败走 `storageError`。
  - `book.open`、`book.openPath`、`book.close`、`chapter.get` 改为结构化错误；空参数走
    `invalidArgument`，未打开书籍走 `notFound`。
  - `annotation.save/list/delete` 与 `reading.saveProgress/getProgress` 改为结构化错误；空 id/bookId
    走 `invalidArgument`，SQLite/锁错误走 `storageError`。
- `src/platform/protocol.ts` 与 `docs/resource-library-plan/8_桥接协议_v0.1.md` 同步说明：
  当前 `opds.*`、`library.*`、`book.*`、`chapter.get`、`annotation.*`、`reading.*`
  已采用 `{ code, message, details? }` 结构化错误形态。
- 本轮不新增错误码、不改版权/获取边界、不改 OPDS acquire 行为。

已验证：

- `cargo check --workspace` 通过。
- `node scripts/check-arch.mjs` 通过。
- `node --check scripts/tauri-opds-smoke.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过；仅有 Windows 换行提示。

下一步：

- 提交并推送 `codex/remaining-bridge-errors`，开 PR。
- 之后继续协议冻结审计：批量/预取语义、资源通道核对，以及后续新增命令是否默认使用 `BridgeError`。

## 2026-06-21：定位升级为轻小说平台

变更：

- 合并 PR #25 到 `main`，使 `library.*` 结构化错误码与上一版 README 进入主线。
- 新建 `codex/platform-positioning`，全面更新产品定位：
  - `README.md`：从“轻小说阅读器”改为“本地优先轻小说平台”，补充平台能力、合规边界、阅读方式选择。
  - `PROJECT_MEMORY.md`：长期定位改为平台；阅读器是核心模块但不是完整边界。
  - `DEVELOPMENT_OUTLINE.md`：当前阶段改为平台化早期实现；新增 v0.6.5“阅读方式选择与合法开放资源获取体验”。
  - `DECISIONS.md`：新增两条决策：产品定位升级为轻小说平台；合法开放资源可站内获取但阅读方式必须由用户选择。
  - `resource-library-plan/0/1/3/4`：总览、产品定位、在线资源接入、合规边界同步平台化与阅读方式模型。
  - `docs/README.md` 与 `AGENTS.md`：入口文档同步平台定位和当前优先级。
  - `NEXT_ACTIONS.md`：新增“给下一轮 Codex”的交接，下一步优先做阅读方式选择动作模型与 UI。
- 明确产品规则：公共版权、开放授权、用户自有资源，以及经 ToS/授权确认可获取的官方免费资源，可以站内获取/
  缓存/阅读；商业、受保护或未知授权正文只保存 metadata 与官方入口。
- 明确阅读方式：浏览器打开、内置阅读器打开、外部本地阅读器打开、获取/缓存后打开。

验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `git diff --check` 通过；仅有 Windows 换行提示。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。

下一步：

- 提交并推送 `codex/platform-positioning`，开 PR。
- 代码层下一步做阅读方式选择动作模型与 UI。

## 2026-06-21：library.* 结构化错误码与 README 更新

变更：

- 合并 PR #24 到 `main`：OPDS 第一批 `BridgeError` 已进入主线。
- 从最新 `main` 新建 `codex/library-bridge-errors`，继续迁移 `library.*` 命令到结构化
  `BridgeError { code, message, details? }`。
- `src-tauri/src/lib.rs`：
  - `library.listCalibre`、`library.import`、`library.importBytes`、`library.list`、`library.search`、
    `library.listSourceRecords`、`library.linkRemoteToLocal`、`library.searchRemote`、
    `library.searchRemoteSource`、`library.acquireRemote`、`library.open`、`library.touchLastRead`
    已从 `Result<_, String>` 迁到 `Result<_, BridgeError>`。
  - 远程搜索按场景映射到 `networkError`、`httpStatus`、`parseError`、`storageError`；
    青空 acquire 的非公共版权/非官方 URL 走 `forbidden`，缺条目/缺正文 URL 走 `notFound`。
- `src/main.ts`：书库相关错误展示改为走 `formatError(e)`，可以展示结构化 `code/details`。
- `src/platform/protocol.ts` 与 `docs/resource-library-plan/8_桥接协议_v0.1.md`：
  更新结构化错误码已覆盖 `opds.*` 与 `library.*` 的说明。
- 根 `README.md` 从早期 EPUB reader 说明更新为当前项目入口，覆盖能力、合规边界、架构、开发命令、
  冒烟/打包命令和当前开发线。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo check --workspace` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过；仅有 Windows 换行提示。

下一步：

- 提交并推送 `codex/library-bridge-errors`，开 PR。
- 后续继续迁移 `book.*`、`annotation.*`、`reading.*`，再收口协议冻结审计。

## 2026-06-21：从 GitHub 同步并补齐 BridgeError 前端消费

变更：

- 按用户要求从 GitHub 同步：`git fetch --all --prune`、`git checkout main`、`git pull --ff-only`
  后确认 `main` 已是最新；随后切回/继续 `codex/structured-opds-errors`，该分支以最新 `main` 为祖先。
- 在 2026-06-19 的 OPDS 结构化错误码第一批基础上，补齐前端真实消费点：
  `src/main.ts` 从 `src/platform` 引入 `isBridgeError`，统一 `formatError(err)` 在识别
  `BridgeError` 时展示 `message`，并附带稳定 `code` 与可选 `details`。
- OPDS 源列表、添加/移除源、浏览/搜索 feed、单条加入书架、下载 EPUB、批量加入书架等错误展示点
  已改为走 `formatError(e)`，不再直接使用 `e?.message || e`。
- `docs/resource-library-plan/8_桥接协议_v0.1.md` 补充：TS 侧 `isBridgeError` 已被
  `src/main.ts::formatError` 消费，当前 OPDS UI 会显示结构化错误码信息。

修改文件：

- `src/main.ts`
- `docs/resource-library-plan/8_桥接协议_v0.1.md`
- `docs/dev-memory/DEV_LOG.md`
- `docs/dev-memory/NEXT_ACTIONS.md`

验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `git diff --check` 通过；仅有 Windows 换行提示。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。

下一步：

- 提交并推送 `codex/structured-opds-errors`，打开/更新 PR。
- 后续继续按价值迁移其余命令到 `BridgeError`，或在 UI 里基于 `code` 做更细的重试/授权/参数错误提示。

## 2026-06-19：结构化错误码（第一批：OPDS 命令）

变更：

- 把先前作为死代码草拟的 `BridgeError`（`src-tauri/src/lib.rs`，`code/message/details`，
  `#[serde(rename_all = "camelCase")]`）正式接线，新增 `forbidden` 构造器，错误码共 7 个：
  `invalidArgument / storageError / parseError / networkError / httpStatus / notFound / forbidden`。
- 把 v0.6 OPDS 网络/存储面 7 个命令的返回类型从 `Result<_, String>` 改为 `Result<_, BridgeError>`，
  按场景映射到对应构造器：`opds_add_source` / `opds_remove_source` / `opds_list_sources` /
  `opds_browse_feed` / `opds_search_feed` / `opds_ingest_entries` / `opds_download_epub`
  （含共享的 `opds_parse_body`）。HTTP 非 2xx 走 `http_status`（details 带状态码），
  reqwest 失败走 `network`，SQLite/锁失败走 `storage`，解析失败走 `parse`，
  空 URL/空查询走 `invalid_argument`，找不到源/条目走 `not_found`，非 open_license 下载走 `forbidden`。
- 其余命令暂保持字符串返回，后续逐步迁移（非破坏性：reject 形态从字符串扩为对象，`message` 仍在）。

协议同步：

- `src/platform/protocol.ts`：新增 `BridgeErrorCode` 联合类型、`BridgeError` 接口、`isBridgeError` 守卫。
  `src/platform/tauri.ts` 无需改动——Tauri `invoke` 直接把序列化错误对象作为 rejection 值，
  前端既有 `e?.message || e` 兼容；要按类别处理可用 `isBridgeError`。
- `docs/resource-library-plan/8_桥接协议_v0.1.md`：原则 4 改写、新增「结构化错误码」一节（错误码清单 +
  已迁移命令列表）、冻结前检查清单第 3 项标记「进行中」。

修改文件：

- `src-tauri/src/lib.rs`
- `src/platform/protocol.ts`
- `docs/resource-library-plan/8_桥接协议_v0.1.md`
- `docs/dev-memory/DEV_LOG.md`
- `docs/dev-memory/NEXT_ACTIONS.md`

验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run build` 通过（tsc + vite build）。
- `cargo test --workspace` 通过（reading-core 全过）。

下一步：

- 按价值把其余命令（library.*、annotation.*、reading.*、book.*）逐步迁移到 `BridgeError`，
  迁移完成后再定稿协议冻结的错误码范围。

## 2026-06-18：真实 OPDS 联网冒烟脚本

变更：

- 新增 `npm.cmd run smoke:opds`，通过 tauri-driver + WebView2 驱动真实 Tauri 窗口跑 OPDS 联网冒烟。
- 冒烟流程覆盖：书架搜索框粘贴 OPDS feed URL 提示 → 填入 OPDS 源 → 添加源 → 浏览 Project Gutenberg OPDS feed → 进入《Pride and Prejudice》单本详情 → 下载 EPUB → 入库 → `library_open` 打开 → 遍历 spine 验证正文可读。
- 默认真实源为 `https://www.gutenberg.org/ebooks/search.opds/?query=austen`；Standard Ebooks 官方页面公开列出的 OPDS 根 feed 在当前网络环境返回认证/HTML，不作为自动冒烟基线。

修改文件：

- `package.json`
- `scripts/tauri-opds-smoke.mjs`
- `docs/dev-memory/DEV_LOG.md`
- `docs/dev-memory/NEXT_ACTIONS.md`

验证：

- `node --check scripts/tauri-opds-smoke.mjs` 通过。
- `npm.cmd run build` 通过。
- `npm.cmd run tauri -- build --debug --no-bundle` 通过，生成 `target/debug/reader.exe`。
- `npm.cmd run smoke:opds` 通过；下载并打开 Project Gutenberg《Pride and Prejudice》。

下一步：

- 跑完整收工验证套件后，把 PR #23 更新为包含 OPDS 冒烟脚本。
- 后续仍可继续推进结构化错误码与协议冻结审计。

## 2026-06-18：OPDS feed URL 粘贴识别

变更：

- 书架搜索框现在会识别看起来像 OPDS/feed/catalog 的 `http(s)` URL，并显示“填入 OPDS 源”提示。
- 点击提示后会展开 OPDS 源面板，把 URL 填入源地址，并用域名预填源名称；仍需用户显式点击“添加 OPDS 源”，不自动联网添加。
- 对 OPDS 源输入框补齐 focus 样式，提示条在窄屏下保持可读。

修改文件：

- `index.html`
- `src/main.ts`
- `src/styles.css`

验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过；仅有 Windows 换行提示。

未验证：

- 尚未在真实 Tauri 窗口里粘贴真实 OPDS feed URL 走完整添加/浏览流程。

下一步：

- 做真实联网 OPDS 冒烟：添加真实 OPDS 目录站点，验证浏览、导航、下载 EPUB、入库打开。
- 继续推进结构化错误码与协议冻结审计。

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

## 2026-06-18：来源记录只读面板

变更：

- 新增 `library.listSourceRecords` 只读协议：reading-core 可按本地 `asset.id` 或远程 `edition.id` 查询同一 edition 的 `source_record`。
- Tauri command 与 TypeScript bridge 同步，前端仍只经 `src/platform/` 访问平台壳。
- 书架卡片新增“来源”按钮，展示来源名称、类型、授权/可用状态、remote id、外链和最近检查时间；该面板不下载正文、不自动合并。
- `library.linkRemoteToLocal` 单测补充来源记录迁移前后查询：远程记录迁到本地 edition 后，本地 asset id 与 edition id 均可查到，远程空壳不再返回。

修改文件：

- `crates/reading-core/src/library.rs`
- `src-tauri/src/lib.rs`
- `src/platform/protocol.ts`
- `src/platform/tauri.ts`
- `src/platform/index.ts`
- `src/main.ts`
- `src/styles.css`
- `docs/resource-library-plan/8_桥接协议_v0.1.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- cargo test -p reading-core 通过（74 passed）；npm.cmd run build 通过（含 check-arch、tsc、vite build）。
- cargo test --workspace 通过（reading-core 74 passed）。
- node scripts/check-arch.mjs 通过。
- node scripts/check-dev-memory.mjs 通过。
- git diff --check 通过（仅 Windows 换行提示）。

未验证/阻塞：

- 无

下一步：

- 继续做关联候选排序/提示或批量人工确认队列；版权边界不变：Bangumi/なろう 只做元数据和外链，library.acquireRemote 仍只允许青空公共版权条目。

## 2026-06-18：关联候选排序与提示

变更：

- 增强远程条目关联本地书面板：候选合并标题搜索与全量本地书后去重，按标题、作者、系列、语言、卷号打分排序；面板展示匹配分数、命中理由、语言/卷号冲突和低置信提醒；低分关联确认时追加人工核对提示；同时修正 linkRemoteEntry 对 isTauriRuntime 的函数调用。

修改文件：

- `src/main.ts`
- `src/styles.css`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- npm.cmd run build 通过（含 check-arch、tsc、vite build）。
- cargo test --workspace 通过（reading-core 74 passed）。
- node scripts/check-dev-memory.mjs 通过。
- git diff --check 通过（仅 Windows 换行提示）。

未验证/阻塞：

- 无

下一步：

- 继续做批量人工确认队列，或给真窗口 smoke:remote-link 补来源面板/候选排序断言；版权边界不变，仍不自动合并、不抓正文。

## 2026-06-18：批量人工确认队列

变更：

- 新增批量人工确认队列第一版：当前书架/远程搜索结果中有远程条目时启用“批量关联”按钮。
- 队列逐条展示远程条目、来源摘要、推荐本地候选、匹配分数/理由/冲突提醒，用户可逐条关联或跳过。
- 关联成功后复用单条关联的 `library.linkRemoteToLocal` 路径迁移 `source_record`；不自动合并、不抓正文。

修改文件：

- `index.html`
- `src/main.ts`
- `src/styles.css`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- npm.cmd run build 通过（含 check-arch、tsc、vite build）。
- cargo test --workspace 通过（reading-core 74 passed）。
- node scripts/check-dev-memory.mjs 通过。
- git diff --check 通过（仅 Windows 换行提示）。

未验证/阻塞：

- 无

下一步：

- 给 smoke:remote-link 补来源面板、候选排序和批量确认队列的真窗口断言；版权边界不变。

## 2026-06-18：实验室电脑交接整理

变更：

- 在 `NEXT_ACTIONS.md` 顶部新增“给实验室电脑 Codex”的交接留言。
- 明确当前工作在 PR #22 / `codex/source-record-panel`，尚未合并进 `main`。
- 汇总 PR #22 的 3 个提交、已完成能力、验证命令和下一步建议。
- 明确下一步优先做真实 Tauri 窗口 `smoke:remote-link` 回归断言，验证通过后再 review/merge PR #22。

修改文件：

- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- `node scripts/check-dev-memory.mjs` 通过。
- `git diff --check` 通过（仅 Windows 换行提示）。

未验证/阻塞：

- 无。

下一步：

- 实验室电脑先 checkout PR #22，再补真窗口 smoke 断言；不要只在 `main` 上找这些未合并功能。

## 2026-06-18：补强 smoke:remote-link 覆盖来源面板/候选排序/批量队列

变更：

- `scripts/tauri-remote-link-smoke.mjs` 改为先在线搜索真实远程条目，再按该远程标题动态生成一个临时本地 EPUB；这样候选排序面板能稳定显示高置信分数与匹配理由。
- smoke 新增断言：
  - 单条“关联本地书”面板显示候选分数、匹配理由，并按分数降序排列。
  - “批量人工确认”队列可打开，候选下拉显示分数，行内显示匹配分数/理由。
  - 批量队列可逐条“跳过”，再逐条“关联”；关联后远程空壳消失，本地进度/标注键仍按本地 `asset.id` 命中。
  - 关联后的本地卡片“来源”面板可打开，能看到 AniList 来源记录、状态信息与外链。

修改文件：

- `scripts/tauri-remote-link-smoke.mjs`
- `docs/dev-memory/DEV_LOG.md`
- `docs/dev-memory/NEXT_ACTIONS.md`

验证：

- `node --check scripts/tauri-remote-link-smoke.mjs` 通过。
- `npm.cmd run tauri -- build --debug --no-bundle` 通过（含 `check-arch`、`tsc`、`vite build`）。
- `npm.cmd run smoke:remote-link -- --tauri-driver C:\Users\41267\.cargo\bin\tauri-driver.exe --native-driver C:\Users\41267\AppData\Local\lightnovel-reader-tools\msedgedriver\149.0.4022.69\msedgedriver.exe` 通过：AniList `Tanya` → `Youjo Senki`，候选面板显示 `匹配 100 · 标题一致 · 作者一致 · 系列一致 · 语言一致`，批量队列完成跳过/关联，来源面板显示 `AniList`。

环境补充：

- 本机缺少 WebDriver 工具，已通过 `cargo install tauri-driver --locked` 安装 `tauri-driver v2.0.6` 到用户 cargo bin。
- 本机 Edge 版本为 `149.0.4022.69`，已从官方 `msedgedriver.microsoft.com` 下载匹配的 `msedgedriver.exe` 到 `%LOCALAPPDATA%\lightnovel-reader-tools\msedgedriver\149.0.4022.69\`；下载需走本机系统代理 `127.0.0.1:7890`。

未验证/阻塞：

- 尚未执行 PR review/merge；下一步做最终检查后 review 并合并 PR #22。

## 2026-06-18：OPDS v0.6 第一轮介入：完成 Rust core OPDS 1.x Atom XML 解析器（connectors::opds，含 OpdsFeed/OpdsEntry/OpdsLink 结构、parse_opds_1x 快速 XML 事件解析器、to_remote_entry 权利状态映射），5 个单测覆盖解析/导航条目/权利映射/边界/端到端落库。新增 6 个 Tauri 命令（opds_add_source/remove_source/list_sources/browse_feed/search_feed/ingest_entries），扩展协议 DTO 与 bridge 方法，前端新增 OPDS 书源管理面板与 feed 浏览器 UI（源增删/浏览/搜索/子分类导航/条目摄入/全部加入书架），支持 relative URL 解析。测试 79 passed 0 failed，前端 tsc + vite build 通过，check-arch/check-dev-memory 通过。

变更：

- OPDS v0.6 第一轮介入：完成 Rust core OPDS 1.x Atom XML 解析器（connectors::opds，含 OpdsFeed/OpdsEntry/OpdsLink 结构、parse_opds_1x 快速 XML 事件解析器、to_remote_entry 权利状态映射），5 个单测覆盖解析/导航条目/权利映射/边界/端到端落库。新增 6 个 Tauri 命令（opds_add_source/remove_source/list_sources/browse_feed/search_feed/ingest_entries），扩展协议 DTO 与 bridge 方法，前端新增 OPDS 书源管理面板与 feed 浏览器 UI（源增删/浏览/搜索/子分类导航/条目摄入/全部加入书架），支持 relative URL 解析。测试 79 passed 0 failed，前端 tsc + vite build 通过，check-arch/check-dev-memory 通过。

修改文件：

- 待补充

验证：

- cargo test --workspace (79 passed), npm run build (tsc + vite), check-arch, check-dev-memory

未验证/阻塞：

- 无

下一步：

- v0.6 OPDS 第二轮：OPDS 2.0 JSON Feed 支持；实机联网冒烟（真实 OPDS 目录站点）；OPDS EPUB 下载 acquire 管线（open_license 条目的正文获取与本地资产转换）；URL 粘贴识别 OPDS feed；结构化错误码与协议冻结审计。

## 2026-06-18：OPDS v0.6 第二轮：OPDS 2.0 JSON Feed 支持。新增 parse_opds_2x(json) 解析器，支持 RWPM/JSON-LD 格式的 navigation/publications/groups/facets 结构、schema.org 元数据映射（author 字符串/对象、identifier 提取、images 封面优先缩略图、acquisition 链接提取）、group 扁平化前缀归并。修复 to_remote_entry 权利映射：acquisition rel 含 borrow/buy/sample 后缀不再误判为 open_license。Tauri opds_browse_feed/search_feed 新增 application/opds+json Accept header + 自动格式检测（sniff 首字符 { vs <）。新增 5 个 OPDS 2.0 单测（导航 feed、出版 feed 元数据/封面/权利、groups 扁平化、边界、端到端落库）。测试 84 passed 0 failed。

变更：

- OPDS v0.6 第二轮：OPDS 2.0 JSON Feed 支持。新增 parse_opds_2x(json) 解析器，支持 RWPM/JSON-LD 格式的 navigation/publications/groups/facets 结构、schema.org 元数据映射（author 字符串/对象、identifier 提取、images 封面优先缩略图、acquisition 链接提取）、group 扁平化前缀归并。修复 to_remote_entry 权利映射：acquisition rel 含 borrow/buy/sample 后缀不再误判为 open_license。Tauri opds_browse_feed/search_feed 新增 application/opds+json Accept header + 自动格式检测（sniff 首字符 { vs <）。新增 5 个 OPDS 2.0 单测（导航 feed、出版 feed 元数据/封面/权利、groups 扁平化、边界、端到端落库）。测试 84 passed 0 failed。

修改文件：

- 待补充

验证：

- cargo test --workspace (84 passed), npm run build (tsc + vite), check-arch/check-dev-memory OK

未验证/阻塞：

- 无

下一步：

- v0.6 第三轮：OPDS EPUB 下载 acquire 管线（open_license 条目 HTTP 下载 → 本地 asset 转换）；实机联网冒烟；URL 粘贴识别；结构化错误码与协议冻结审计。

## 2026-06-18：v0.6 OPDS 第三轮：EPUB 下载 acquire 管线。新增 library::attach_remote_epub_bytes（直接保存下载的 EPUB 字节为本地 asset，含元数据提取/封面缩略图/DB 写入）、opds_download_epub Tauri 命令（HTTP GET → 校验→ 落库）、协议 bridge 方法 opdsDownloadEpub、前端下载按钮（两步骤：摄入→下载，含状态反馈）。与 aozora acquire 不同：OPDS 直接提供预建 EPUB，无需 HTML→EPUB 合成。

变更：

- v0.6 OPDS 第三轮：EPUB 下载 acquire 管线。新增 library::attach_remote_epub_bytes（直接保存下载的 EPUB 字节为本地 asset，含元数据提取/封面缩略图/DB 写入）、opds_download_epub Tauri 命令（HTTP GET → 校验→ 落库）、协议 bridge 方法 opdsDownloadEpub、前端下载按钮（两步骤：摄入→下载，含状态反馈）。与 aozora acquire 不同：OPDS 直接提供预建 EPUB，无需 HTML→EPUB 合成。

修改文件：

- 待补充

验证：

- cargo test --workspace (84 passed), npm run build (tsc + vite), check-arch, check-dev-memory

未验证/阻塞：

- 无

下一步：

- v0.6 第四轮：实机联网冒烟（真实 OPDS 站点测试完整流程）；URL 粘贴识别 OPDS feed；结构化错误码与协议冻结审计。

## 2026-06-21：阅读方式选择第一版：书架卡片新增内置/外部/获取/浏览器动作；新增 ReaderBridge shell.openPathExternal，并在 Tauri 平台层映射到 @tauri-apps/plugin-opener openPath；README 与桥接协议文档已同步。

变更：

- 阅读方式选择第一版：书架卡片新增内置/外部/获取/浏览器动作；新增 ReaderBridge shell.openPathExternal，并在 Tauri 平台层映射到 @tauri-apps/plugin-opener openPath；README 与桥接协议文档已同步。

修改文件：

- 待补充

验证：

- npm.cmd run build 通过（含 check-arch、tsc、vite build）。

未验证/阻塞：

- 无

下一步：

- 继续跑 check-dev-memory、cargo test --workspace、git diff --check；若通过则提交并开 PR。后续可做阅读方式偏好持久化与 OPDS open_license 统一 acquire 动作。

## 2026-06-21：阅读方式选择第一版收工补记：功能差异保持不变，NEXT_ACTIONS 顶部已从进行中改为已完成交接。

变更：

- 阅读方式选择第一版收工补记：功能差异保持不变，NEXT_ACTIONS 顶部已从进行中改为已完成交接。

修改文件：

- 待补充

验证：

- node scripts/check-dev-memory.mjs 通过；cargo test --workspace 通过（reading-core 84 passed）；git diff --check 通过（仅 Windows 换行提示）；此前 npm.cmd run build 已通过。

未验证/阻塞：

- 无

下一步：

- 提交并推送 codex/reading-action-model，打开 PR；后续做阅读方式偏好持久化、OPDS open_license 与公共版权 acquire 的统一动作入口。

## 2026-06-21：阅读方式偏好持久化：书库标题栏新增默认阅读方式选择（自动/内置/浏览器/外部），偏好写入 localStorage；书架卡片主按钮与卡片点击按偏好选择可用动作并自动回退。

变更：

- 阅读方式偏好持久化：书库标题栏新增默认阅读方式选择（自动/内置/浏览器/外部），偏好写入 localStorage；书架卡片主按钮与卡片点击按偏好选择可用动作并自动回退。

修改文件：

- 待补充

验证：

- npm.cmd run build 通过（含 check-arch、tsc、vite build）。

未验证/阻塞：

- 无

下一步：

- 跑 check-dev-memory、cargo test --workspace、git diff --check；后续优先统一 OPDS open_license 与青空 public_domain acquire/open 动作入口，或继续迁移 book/annotation/reading 到 BridgeError。

## 2026-06-21：阅读方式偏好收工补记：功能差异保持不变，PROJECT_MEMORY / README / NEXT_ACTIONS 已同步当前状态。

变更：

- 阅读方式偏好收工补记：功能差异保持不变，PROJECT_MEMORY / README / NEXT_ACTIONS 已同步当前状态。

修改文件：

- 待补充

验证：

- node scripts/check-dev-memory.mjs 通过；cargo test --workspace 通过（reading-core 84 passed）；git diff --check 通过（仅 Windows 换行提示）；此前 npm.cmd run build 已通过。

未验证/阻塞：

- 无

下一步：

- 提交并推送 codex/reading-preference，打开 PR；后续优先统一 OPDS open_license 与青空 public_domain acquire/open 动作入口，或继续迁移 book/annotation/reading 到 BridgeError。

## 2026-06-21：统一合法资源获取后阅读第一步：青空 public_domain 获取后改走 openAcquiredLibraryBook，按默认阅读方式在内置/外部间打开；OPDS open_license 按钮从下载 EPUB 改为获取并阅读，落库、下载 attach 后复用同一打开动作。

变更：

- 统一合法资源获取后阅读第一步：青空 public_domain 获取后改走 openAcquiredLibraryBook，按默认阅读方式在内置/外部间打开；OPDS open_license 按钮从下载 EPUB 改为获取并阅读，落库、下载 attach 后复用同一打开动作。

修改文件：

- 待补充

验证：

- npm.cmd run build 通过（含 check-arch、tsc、vite build）。

未验证/阻塞：

- 无

下一步：

- 跑 check-dev-memory、cargo test --workspace、git diff --check；后续做真实 Tauri/OPDS 冒烟，并评估 acquisition URL 持久化以支持从书架远程 OPDS 条目直接获取。

## 2026-06-21：统一合法资源获取后阅读收工补记：功能差异保持不变，README / PROJECT_MEMORY / NEXT_ACTIONS 已同步。

变更：

- 统一合法资源获取后阅读收工补记：功能差异保持不变，README / PROJECT_MEMORY / NEXT_ACTIONS 已同步。

修改文件：

- 待补充

验证：

- node scripts/check-dev-memory.mjs 通过；cargo test --workspace 通过（reading-core 84 passed）；git diff --check 通过（仅 Windows 换行提示）；此前 npm.cmd run build 已通过。

未验证/阻塞：

- 无

下一步：

- 提交并推送 codex/unified-acquire-open，打开 PR；后续做真实 Tauri/OPDS 冒烟，并设计 acquisition URL 持久化以支持书架远程 OPDS 条目直接获取。

## 2026-06-21：OPDS 获取并阅读真实冒烟补验

变更：

- `scripts/tauri-opds-smoke.mjs` 兼容新的“获取并阅读”按钮，并保留旧 `EPUB` 文本兼容。
- OPDS 冒烟新增阅读态断言：获取 Gutenberg EPUB 后必须进入 `reading-active`，书库层隐藏，状态栏可见。
- 修复真实冒烟发现的问题：OPDS 获取后先打开阅读器、再刷新书库会让书库层覆盖阅读器；现在先刷新书库，再按默认阅读方式打开获取后的本地 asset。
- `PROJECT_MEMORY.md` / `NEXT_ACTIONS.md` / `发布与测试.md` 已记录 `smoke:opds` 的真实验证范围和后续任务。

修改文件：

- `src/main.ts`
- `scripts/tauri-opds-smoke.mjs`
- `docs/dev-memory/PROJECT_MEMORY.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`
- `docs/current-project/发布与测试.md`

验证：

- `node --check scripts/tauri-opds-smoke.mjs` 通过。
- `npm.cmd run tauri -- build --debug --no-bundle` 通过。
- `npm.cmd run smoke:opds -- --tauri-driver C:\Users\Administrator\.cargo\bin\tauri-driver.exe --native-driver C:\Users\Administrator\AppData\Local\lightnovel-reader-tools\msedgedriver\149.0.4022.62\msedgedriver.exe` 通过（Gutenberg OPDS → Pride and Prejudice → 获取并阅读，`readerUi` 为 `libraryHidden=true` / `readingActive=true` / `statusbarHidden=false`）。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过（仅 Windows 换行提示）。

未验证/阻塞：

- 无

下一步：

- 提交并推送 codex/opds-acquire-smoke，打开 PR；后续可设计 acquisition URL 持久化，支持从书架远程 OPDS 条目直接获取并阅读。

## 2026-06-21：OPDS acquisition URL 持久化

变更：

- `source_record` 新增迁移 v6：`acquisition_url TEXT`，用于保存合法开放正文获取链接；`remote_url` 继续只表示官方/来源页面外链。
- `RemoteEntry` / `connectors::ingest` / `LibraryBook` / `LibrarySourceRecord` / `RemoteAcquisition` 已贯通 `acquisitionUrl`。
- OPDS feed 入库前会解析相对链接，开放授权条目的 `acquisitionUrl` 会随来源记录持久化。
- `opds.downloadEpub(editionId, acquisitionUrl?)` 第二参数改为可选；未传时从库内 `source_record.acquisition_url` 回读，并继续强制 `rightsStatus=open_license`。
- 书架远程 OPDS `open_license` 条目现在可直接显示“获取”动作，获取后按阅读偏好打开。
- README、桥接协议文档、schema 草案已同步。

修改文件：

- `crates/reading-core/src/connectors.rs`
- `crates/reading-core/src/library.rs`
- `src-tauri/src/lib.rs`
- `src/platform/protocol.ts`
- `src/main.ts`
- `scripts/tauri-opds-smoke.mjs`
- `README.md`
- `docs/resource-library-plan/8_桥接协议_v0.1.md`
- `docs/resource-library-plan/10_书库实体模型_v0.5_schema草案.md`
- `docs/current-project/发布与测试.md`
- `docs/dev-memory/PROJECT_MEMORY.md`
- `docs/dev-memory/NEXT_ACTIONS.md`
- `docs/dev-memory/DEV_LOG.md`

验证：

- `cargo check --workspace` 通过。
- `npm.cmd run build` 通过。
- `node --check scripts/tauri-opds-smoke.mjs` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `npm.cmd run tauri -- build --debug --no-bundle` 通过。
- `npm.cmd run smoke:opds -- --tauri-driver C:\Users\Administrator\.cargo\bin\tauri-driver.exe --native-driver C:\Users\Administrator\AppData\Local\lightnovel-reader-tools\msedgedriver\149.0.4022.62\msedgedriver.exe` 通过：Gutenberg OPDS → Pride and Prejudice → 加入书架（`acquisitionUrl=https://www.gutenberg.org/ebooks/1342.epub.noimages`）→ 从书架卡片获取并阅读 → 进入阅读态。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `git diff --check` 通过（仅 Windows 换行提示）。

未验证/阻塞：

- 无。

下一步：

- 继续迁移 `book.*` / `annotation.*` / `reading.*` 到结构化 `BridgeError`；若准备分发，重跑 `package:beta` 并做解压/启动检查。

## 2026-06-27：Hermes 三层架构三线并行：1) Claude Code 完成 QuickJS 集成架构方案(187行)；2) OpenCode+Sonnet 完成插件仓库 smoke 测试(592行) + 限制说明文档；3) 更新 AGENTS.md 当前优先级

变更：

- Hermes 三层架构三线并行：1) Claude Code 完成 QuickJS 集成架构方案(187行)；2) OpenCode+Sonnet 完成插件仓库 smoke 测试(592行) + 限制说明文档；3) 更新 AGENTS.md 当前优先级

修改文件：

- 待补充

验证：

- check-arch/check-dev-memory/check-protocol-freeze/node--check 全绿

未验证/阻塞：

- 无

下一步：

- review QuickJS 集成方案文档 → 确认选型后进入实现；review smoke 测试 → 补真实窗口运行

## 2026-06-27：Phase 1 WASM 网页端 MVP 完工：reading-core 拆 native/wasm features，新增 Rust 分页算法 pagination.rs(347行/8测试)，WASM 编译导出 paginate/parse_epub_metadata/get_chapter_html，Web Worker 接入 WASM 分页(TS fallback)，新建 web-storage(IndexedDB+OPFS)/web-bridge(ReaderBridge 229行)/web-import(拖放+文件选择)，platform/index.ts 浏览器模式切换 webBridge(替代全抛错的 noBridge)

变更：

- Phase 1 WASM 网页端 MVP 完工：reading-core 拆 native/wasm features，新增 Rust 分页算法 pagination.rs(347行/8测试)，WASM 编译导出 paginate/parse_epub_metadata/get_chapter_html，Web Worker 接入 WASM 分页(TS fallback)，新建 web-storage(IndexedDB+OPFS)/web-bridge(ReaderBridge 229行)/web-import(拖放+文件选择)，platform/index.ts 浏览器模式切换 webBridge(替代全抛错的 noBridge)

修改文件：

- 待补充

验证：

- npm run build: 18 modules/500KB WASM 通过；cargo test --workspace: 131 passed；tsc --noEmit: 零错误；check-arch/check-protocol-freeze: OK

未验证/阻塞：

- 无

下一步：

- Phase 2: 自托管同步服务(axum+sync_outbox)；Phase 3: 桌面端独立化(托盘/文件关联/自动更新)

## 2026-06-27：Phase 2 同步服务 v1: reading-core::sync 模块(277行/6测试)含冲突解决算法(LWW+墓碑复活); migration v7(library DB:edition/asset加sync列+sync_outbox表+触发器)+storage v2(annotations/reading_state加sync列); 新建crates/sync-server(axum+SQLite,457行)含/pair,/sync/changes,/sync/snapshot,/sync/push,/sync/blobs,WebSocket推送; 桥接协议新增syncPair/syncStatus/syncNow/syncUnpair; web-bridge实现sync pair+localStorage存凭据; tauri.ts sync stub; src/web/sync-pairing.ts配对UI

变更：

- Phase 2 同步服务 v1: reading-core::sync 模块(277行/6测试)含冲突解决算法(LWW+墓碑复活); migration v7(library DB:edition/asset加sync列+sync_outbox表+触发器)+storage v2(annotations/reading_state加sync列); 新建crates/sync-server(axum+SQLite,457行)含/pair,/sync/changes,/sync/snapshot,/sync/push,/sync/blobs,WebSocket推送; 桥接协议新增syncPair/syncStatus/syncNow/syncUnpair; web-bridge实现sync pair+localStorage存凭据; tauri.ts sync stub; src/web/sync-pairing.ts配对UI

修改文件：

- 待补充

验证：

- cargo test:137 passed
- npm run build:18 modules OK
- tsc:零错误
- check-arch/check-protocol-freeze:OK

未验证/阻塞：

- 无

下一步：

- Phase 3:桌面端独立化(托盘/文件关联/自动更新)
- Tauri端实现sync命令对接sync-server
- 网页端syncNow轮询实现

## 2026-06-27：Phase 3 桌面端独立化: 系统托盘(tauri tray-icon feature, 双击恢复/菜单显示+退出); 关闭到托盘(prevent_close+hide); 启动优化(window visible:false + 200ms延迟show); .epub文件关联(tauri.conf.json fileAssociations); 自动更新(tauri-plugin-updater, GitHub releases endpoint, passive install); 命令行参数读入.epub自动打开

变更：

- Phase 3 桌面端独立化: 系统托盘(tauri tray-icon feature, 双击恢复/菜单显示+退出); 关闭到托盘(prevent_close+hide); 启动优化(window visible:false + 200ms延迟show); .epub文件关联(tauri.conf.json fileAssociations); 自动更新(tauri-plugin-updater, GitHub releases endpoint, passive install); 命令行参数读入.epub自动打开

修改文件：

- 待补充

验证：

- cargo check:OK
- cargo test:137 passed
- npm run build:18 modules OK
- check-arch/check-protocol-freeze:OK

未验证/阻塞：

- 无

下一步：

- Phase 4: GPU翻页动画(CSS transform双缓冲)+PWA接入
- Tauri sync命令对接sync-server
- 冷启动计时验收

## 2026-06-28：Phase 4 性能打磨+ PWA: GPU翻页动画(CSS transform双缓冲,220ms cubic-bezier,requestAnimationFrame驱动,快速翻页直接更新弃层); PWA接入(vite-plugin-pwa,autoUpdate,16 precache条目含WASM/JS/CSS/图标,standalone模式); reader-core.ts:renderCurrentPages改为双缓冲架构(_activeLayer/_nextLayer+_pageAnimating标志+transitionend事件交换角色)

变更：

- Phase 4 性能打磨+ PWA: GPU翻页动画(CSS transform双缓冲,220ms cubic-bezier,requestAnimationFrame驱动,快速翻页直接更新弃层); PWA接入(vite-plugin-pwa,autoUpdate,16 precache条目含WASM/JS/CSS/图标,standalone模式); reader-core.ts:renderCurrentPages改为双缓冲架构(_activeLayer/_nextLayer+_pageAnimating标志+transitionend事件交换角色)

修改文件：

- 待补充

验证：

- npm run build:18 modules+PWA sw.js+manifest OK
- cargo test:137 passed
- tsc零错误
- check-arch/check-protocol-freeze:OK

未验证/阻塞：

- 无

下一步：

- Tauri sync命令对接sync-server
- 冷启动计时验收
- QuickJS运行时集成

## 2026-06-28：冷启动计时:lib.rs setup添加startup日志(Instant计时+eprintln输出窗口可见时间);scripts/measure-cold-start.mjs冷启动测量脚本(3轮/杀进程/3s等待)

变更：

- 冷启动计时:lib.rs setup添加startup日志(Instant计时+eprintln输出窗口可见时间);scripts/measure-cold-start.mjs冷启动测量脚本(3轮/杀进程/3s等待)

修改文件：

- 待补充

验证：

- cargo check+ npm run build:OK

未验证/阻塞：

- 无

下一步：

- QuickJS运行时集成(Claude Code进行中)

## 2026-06-28：QuickJS插件运行时:新增plugin_runtime.rs(250行),rquickjs v0.9依赖;每次调用创建一次性Runtime+Context保证隔离;注入host.http(domains白名单校验)/host.kv(内存Map,TODO落盘)/host.log+URL/TextDecoder polyfill;PluginHttpExecutor trait让壳层实现HTTP转发;无quickjs feature时提供空壳stub; native feature另含quickjs子feature(cargo --features 'native,quickjs'启用)

变更：

- QuickJS插件运行时:新增plugin_runtime.rs(250行),rquickjs v0.9依赖;每次调用创建一次性Runtime+Context保证隔离;注入host.http(domains白名单校验)/host.kv(内存Map,TODO落盘)/host.log+URL/TextDecoder polyfill;PluginHttpExecutor trait让壳层实现HTTP转发;无quickjs feature时提供空壳stub; native feature另含quickjs子feature(cargo --features 'native,quickjs'启用)

修改文件：

- 待补充

验证：

- cargo check:三crate全过
- cargo test:137 passed
- npm run build:PWA 16条目OK
- rquickjs编译成功

未验证/阻塞：

- 无

下一步：

- 壳层实现PluginHttpExecutor(reqwest转发)
- host.kv持久化到plugin_store
- QuickJS中断超时
- 写集成测试

## 2026-06-28：QuickJS完善: plugin_executor.rs(reqwest blocking HTTP 25s超时转发); plugin_runtime.rs 25s中断线程(spawn sleep+interrupt); Cargo.toml加reqwest blocking feature

变更：

- QuickJS完善: plugin_executor.rs(reqwest blocking HTTP 25s超时转发); plugin_runtime.rs 25s中断线程(spawn sleep+interrupt); Cargo.toml加reqwest blocking feature

修改文件：

- 待补充

验证：

- cargo check:0 error
- cargo test:137 passed
- npm build:OK

未验证/阻塞：

- 无

下一步：

- host.kv持久化到plugin_store
- plugin runtime集成测试
- 前端UI调QuickJS执行

## 2026-06-28：host.kv持久化:plugin_store新增plugin_kv_get/set/delete(kv.json per插件,key≤128字符,value≤64KiB);PluginRuntime加plugin_root+plugin_id字段;inject_host_api使用持久化kv(替代内存Map);空壳stub同步更新签名

变更：

- host.kv持久化:plugin_store新增plugin_kv_get/set/delete(kv.json per插件,key≤128字符,value≤64KiB);PluginRuntime加plugin_root+plugin_id字段;inject_host_api使用持久化kv(替代内存Map);空壳stub同步更新签名

修改文件：

- 待补充

验证：

- cargo test:137 passed
- cargo check:0 error

未验证/阻塞：

- 无

下一步：

- 前端UI调QuickJS执行插件
- 集成测试
- Tauri sync命令完成

## 2026-06-28：Tauri sync命令对接sync-server:sync_commands.rs(200行,6个命令) sync_status/sync_pair/sync_pair_join/sync_unpair/sync_push/sync_pull;凭据存app_data/sync.json;app.manage(dir)为sync命令提供数据目录;reqwest加json feature

变更：

- Tauri sync命令对接sync-server:sync_commands.rs(200行,6个命令) sync_status/sync_pair/sync_pair_join/sync_unpair/sync_push/sync_pull;凭据存app_data/sync.json;app.manage(dir)为sync命令提供数据目录;reqwest加json feature

修改文件：

- 待补充

验证：

- cargo test:137 passed
- cargo check:0 error

未验证/阻塞：

- 无

下一步：

- 前端tauri.ts接入sync命令
- sync-pairing UI对接
- 端到端桌面+网页同步测试

## 2026-06-28：前端sync对接:tauri.ts替换sync stub为真实invoke(sync_pair_join/sync_status/sync_push/sync_pull/sync_unpair);syncPair通过localStorage获取serverUrl;syncNow执行push+pull;syncUnpair清理localStorage

变更：

- 前端sync对接:tauri.ts替换sync stub为真实invoke(sync_pair_join/sync_status/sync_push/sync_pull/sync_unpair);syncPair通过localStorage获取serverUrl;syncNow执行push+pull;syncUnpair清理localStorage

修改文件：

- 待补充

验证：

- tsc零错误
- npm run build:18 modules OK

未验证/阻塞：

- 无

下一步：

- 端到端桌面+网页同步实测(需运行sync-server)
- syncNow自动同步轮询

## 2026-06-28：QuickJS插件测试UI:已安装插件列表加'测试'按钮;testPluginRun()调plugin_test_run命令(走platform/tauri invoke,不碰@tauri-apps);Rust侧新增plugin_test_run命令(读entry.js→PluginRuntime→call方法);tauri.ts导出invoke

变更：

- QuickJS插件测试UI:已安装插件列表加'测试'按钮;testPluginRun()调plugin_test_run命令(走platform/tauri invoke,不碰@tauri-apps);Rust侧新增plugin_test_run命令(读entry.js→PluginRuntime→call方法);tauri.ts导出invoke

修改文件：

- 待补充

验证：

- npm run build:check-arch OK+18 modules+PWA
- cargo test:137 passed

未验证/阻塞：

- 无

下一步：

- 插件的search/getBook/getChapter完整流程
- 插件运行结果UI展示
- 自动同步轮询

## 2026-06-29：测试插件:scripts/test-plugin/含manifest.json+index.js(search/getBook/getChapter+host.kv+host.log硬编码测试数据),scripts/package-test-plugin.mjs打包为test-plugin-hello.zip(1.4KB),可安装到Tauri应用中点'测试'按钮验证QuickJS全流程

变更：

- 测试插件:scripts/test-plugin/含manifest.json+index.js(search/getBook/getChapter+host.kv+host.log硬编码测试数据),scripts/package-test-plugin.mjs打包为test-plugin-hello.zip(1.4KB),可安装到Tauri应用中点'测试'按钮验证QuickJS全流程

修改文件：

- 待补充

验证：

- npm run build:OK
- zip打包成功1.4KB

未验证/阻塞：

- 无

下一步：

- Tauri应用中安装test-plugin-hello.zip→点测试按钮验证
- 写自动化集成测试调plugin_test_run

## 2026-07-21：增加正式发布信任门并评估嵌入式 WDIO

变更：

- 新增 `scripts/check-release-trust.mjs` 与 `scripts/test-release-trust.mjs`，检查官方插件强制验签、Ed25519 公钥 keyring 和 Tauri updater 公钥。
- `package:beta`、`installer:web` 的 npm pre-hook 及 `release:build` 接入门禁；开发构建不受影响。
- 建立 v0.7 检查点提交 `8a8f83f`。

验证：

- `npm.cmd run test:release-trust`：通过。
- 当前仓库 `npm.cmd run check:release-trust`：按设计阻断三项未配置状态。
- `cargo check -p reader`：通过。
- `cargo test --workspace`：通过（Tauri 7 passed / 1 个公网测试 ignored，reading-core 149 passed）。
- `npm.cmd run build` 与 `npm.cmd run check:project`：通过。
- WDIO embedded spike 完成依赖、Rust 插件和真实会话调研，但因上游 `native-utils` 导出缺失、Windows EdgeDriver
  版本解析错误及嵌入会话不稳定未达到提交门槛；相关实验代码和依赖已撤回。

未验证 / 阻塞：

- 正式插件发布公钥、updater 公钥及对应私钥秘密管理尚未由维护者配置，因此正式分发门禁保持红灯。
- 真实 HTTPS 官方仓库、正常公网 DNS 下 Gutenberg 以及最终安装/更新演练仍待具备外部条件后执行。

下一步：

- 维护者安全生成并分别管理插件发布密钥与 updater 密钥，只把公钥注入仓库。
- 签署官方仓库全部 zip、开启强制验签，跑受控 HTTPS 与正式分发演练。

## 2026-07-25：接入首批插件与 updater 正式公钥

变更：

- `src-tauri/src/plugin_trust.rs` 新增 `lnr-plugin-2026-01` Ed25519 公钥，并开启官方仓库强制验签。
- `src-tauri/tauri.conf.json` 接入独立 updater 公钥，启用 `bundle.createUpdaterArtifacts=true`。
- 发布门新增 updater 产物开关检查；单测覆盖四项未配置状态。
- 同步 AGENTS、项目长期记忆、决策、下一步队列与发布说明。

验证：

- `node --check scripts/check-release-trust.mjs`：通过。
- `node --check scripts/test-release-trust.mjs`：通过。
- `npm.cmd run test:release-trust`：通过。
- `npm.cmd run check:release-trust`：通过，识别插件 keyId、updater 公钥和签名产物开关。
- `cargo check -p reader`：通过。
- `npm.cmd run build` 与 `npm.cmd run check:project`：通过。
- `cargo test --workspace`：通过（Tauri 7 passed / 1 个公网测试 ignored，reading-core 149 passed）。
- `npm.cmd run smoke:plugin-repository-signature`：通过，覆盖原始 zip 签名、单字节篡改、错公钥和 core/Tauri 校验链。
- `npm.cmd run tauri -- build --debug --no-bundle`：通过，确认 Tauri 接受 updater 产物配置且不需要读取正式私钥即可完成非分发构建。

未验证 / 阻塞：

- Codex 未读取插件/updater 私钥或 updater 密码，因此未执行真实私钥签名和正式 updater release build。
- 官方仓库最终 zip/URL 尚未确定，真实 HTTPS 签名仓库与 `latest.json` 更新链尚未演练。

下一步：

- 由维护者在仓库外用插件私钥签署首批正式 repository。
- 在受控环境注入 updater 私钥与密码，构建 `.sig` 并完成旧版本到新版本的真实更新测试。

## 2026-07-25：生成首个正式密钥签名仓库候选并补齐发布工具

变更：

- 新增 `scripts/prepare-plugin-repository-release.mjs`：从 zip 内唯一 manifest 生成 unsigned 索引，
  复制包并计算 SHA-256/大小，默认拒绝覆盖。
- `scripts/sign-plugin-repository.mjs` 新增预期公钥参数，拿错私钥时在写出签名索引前失败。
- 新增 `scripts/verify-plugin-repository-release.mjs`：仅用公钥独立复核包哈希、大小、keyId 和签名。
- 用仓库外正式私钥签署 `gutenberg-test@0.1.0` 候选，公钥匹配 `lnr-plugin-2026-01`；
  候选指向未来统一 `v0.3.1` GitHub Release，仅留在仓库外暂存，未上传。
- 新增 `scripts/build-signed-updater.ps1`，以隐藏输入提示 updater 密码并在退出时清理环境变量/密码缓冲。
- 新增 `scripts/prepare-updater-release.mjs`，从实际 NSIS 安装器与 `.sig` 生成 Tauri 静态
  `latest.json`；版本默认读取并强制匹配 Tauri 配置。
- 更新 SDK 发布说明、发布测试入口、项目记忆和决策记录。

验证：

- 新增/修改 JavaScript 脚本 `node --check`：通过。
- 发布准备夹具：通过；确认从真实 zip manifest 生成索引、哈希正确且重复输出被拒绝。
- updater 发布夹具：通过；确认安装器 URL 编码、`.sig` 内容嵌入 `latest.json`。
- updater 版本漂移门：通过；显式版本与 Tauri `0.3.1` 不一致时在写出前失败。
- `npm.cmd run smoke:plugin-repository-signature`：通过；新增公钥匹配门与独立公钥验收覆盖，
  同时通过 reading-core/Tauri 验签测试。
- 正式候选执行 `verify:plugin-repository-release`：通过，1 个条目使用 `lnr-plugin-2026-01`。
- `npm.cmd run check:project`、`npm.cmd run check:release-trust` 与 `npm.cmd run build`：通过。
- `cargo test --workspace`：通过（Tauri 7 passed / 1 个公网测试 ignored，reading-core 149 passed）。
- `git diff --check`：通过（仅 Windows LF→CRLF 提示）。

未验证 / 阻塞：

- updater 私钥带密码；Codex 未读取密码，正式 `release:build` 尚未运行。
- 候选尚未在正常公网 DNS 下复验，也未创建或上传 GitHub Release。
- `gutenberg-test` 仍以测试示例命名；公开前需决定提升为正式来源还是只作为预发布资产。

下一步：

- 维护者在本机运行交互式 updater 构建脚本并输入密码。
- 生成 `v0.3.1` updater 候选、验收安装/更新链后再公开统一 GitHub Release。

## 2026-07-25：定位首次 updater 构建失败并收窄 NSIS 主路径

事实与修复：

- 维护者运行 `build-signed-updater.ps1`；Tauri 接受了 updater 私钥密码，发布信任门、前端构建与 Rust
  release 编译均通过。
- 首次失败发生在 WiX `light.exe`。手动详细复现得到 `LGHT0311`：默认 `en-US / code page 1252`
  无法编码文件关联中文；`tauri.conf.json` 已新增 `windows.wix.language = "zh-CN"`。
- 中文代码页生效后，手动 linker 显示本机 Windows Installer 服务不可访问，ICE01–ICE09 报
  `LGHT0217`；这是 MSI 环境问题，不是密钥或应用编译问题。
- 新增 `release:build:updater`，只构建 NSIS；交互式脚本改用该入口，并把提示/错误文本改为 ASCII，
  避免 Windows PowerShell 5 对无 BOM UTF-8 脚本显示乱码。

验证：

- `npm.cmd run tauri -- build --bundles msi --no-sign`：WiX 中文文件名已生成，1252 错误消失；
  仍被本机 Windows Installer 服务的 ICE 验证阻断。
- `npm.cmd run tauri -- build --bundles nsis --no-sign`：通过，生成
  `target/release/bundle/nsis/LightNovel Reader_0.3.1_x64-setup.exe`。
- Tauri 官方 NSIS 3.11 与 `nsis_tauri_utils` 已下载并完成哈希校验。
- `npm.cmd run check:project` 与 `npm.cmd run build`：通过。
- `cargo test --workspace`：通过（Tauri 7 passed / 1 个公网测试 ignored，reading-core 149 passed）。

未验证 / 阻塞：

- 本轮 NSIS 使用 `--no-sign` 诊断，因此尚无正式 `.sig`；维护者需重新运行交互式签名脚本。
- MSI 仍需修复本机 Windows Installer 服务后复验，但不阻断 updater 主路径。

## 2026-07-25：修正 updater 构建私钥环境变量

事实与修复：

- 维护者再次运行交互式脚本；发布门、前端构建、Rust release 编译和 NSIS 打包均通过，但生成 updater
  签名时报告已找到公钥、未找到私钥。
- 根因不是密码错误：独立 `tauri signer sign` 支持 `TAURI_SIGNING_PRIVATE_KEY_PATH`，但
  `tauri build` 的 bundler 阶段实际读取 `TAURI_SIGNING_PRIVATE_KEY`。
- `build-signed-updater.ps1` 现把同一个仓库外私钥路径同时注入两个兼容变量，并在成功或失败时同时清理；
  密码继续只通过隐藏提示进入进程环境，并清理非托管缓冲。

验证：

- `npm.cmd run tauri -- signer sign --help`：确认 signer CLI 同时声明
  `TAURI_SIGNING_PRIVATE_KEY` 与 `TAURI_SIGNING_PRIVATE_KEY_PATH`。
- PowerShell 语法解析、`check-arch`、`check-dev-memory`、`check:release-trust` 与
  `git diff --check`：通过。
- 修正后的 `build-signed-updater.ps1`：通过；release 前端与 Rust 编译、NSIS 打包和 Tauri updater
  签名均成功，生成安装器与 432 字节 `.sig`。
- `prepare:updater-release`：通过；`latest.json` 版本为 `0.3.1`，URL 指向统一 `v0.3.1` Release，
  内嵌签名与 `.sig` 一致，候选与源安装器/签名哈希一致。
- `verify:plugin-repository-release`：通过；1 个条目由 `lnr-plugin-2026-01` 验签。
- 已组装仓库外 `v0.3.1-release` 统一候选，只含五个公开文件，不含私钥、密码或 unsigned 索引。

下一步：

- 在正常公网 DNS 下复验 Gutenberg 和 NSIS 数据保留；决定插件测试资产定位后再上传统一 GitHub Release，
  并从旧版本执行真实在线更新。

## 2026-07-25：完成正式 NSIS 数据保留验收并复查 Gutenberg 网络阻塞

验证：

- 正式统一候选 NSIS 静默安装退出码 0，安装目录包含 `reader.exe` 与 `uninstall.exe`。
- 安装版成功启动，并在默认 `%APPDATA%\com.lightnovel.reader` 创建真实 `reader.db` 和
  `library\library.sqlite`。
- 静默卸载退出码 0；安装目录移除，开始菜单/桌面无残留 LightNovel Reader 快捷方式，Reader 进程为 0。
- 卸载前后默认应用数据均为 2 个文件；相对路径、长度和 SHA-256 清单完全一致，确认卸载保留用户数据。
- 主机网络对 Gutenberg OPDS HEAD 请求返回 HTTP 200，但 FlClash 虚拟网卡 DNS `198.18.0.2`
  把域名解析为 fake-IP `198.18.0.4`。
- WLAN DNS `192.168.3.1` 与 Google DNS-over-HTTPS 均返回真实公网 IP `152.19.134.47`，确认
  Gutenberg 本身和上游 DNS 正常，阻塞来自本机 FlClash fake-IP 模式。
- `cargo test -p reader runs_gutenberg_search_book_chapter_acquire_flow -- --ignored --nocapture`：
  失败于“插件 HTTP 禁止访问本机或内网地址”；这是 SSRF 防护对保留地址的预期拒绝，不是搜索/解析断言失败。

结论：

- NSIS 安装/启动/卸载与数据保留公开前验收通过。
- 不为适配当前 DNS 映射而削弱 SSRF 防护；应在 FlClash 为 `gutenberg.org` 配置
  real-IP/`fake-ip-filter` 或暂时退出其虚拟 DNS，再复验搜索、预览、获取。
- GitHub Release 尚未创建，旧版本真实在线更新仍未验证。

下一步：

- 换用正常公网 DNS 环境完成 Gutenberg 全流程；确定插件资产定位后再发布统一 GitHub Release，并执行旧版本更新。

## 2026-07-26：完成 Gutenberg 公网闭环并生成 0.1.1 统一 RC2

事实与修复：

- FlClash 开启 DNS 覆写、保留 `+.gutenberg.org` fake-IP 排除并重启后，
  `gutenberg.org` 与 `www.gutenberg.org` 均解析到真实公网 IP `152.19.134.47`；
  SSRF 保留地址拒绝规则未放宽。
- 首次真实公网测试到达 Gutenberg 后发现旧 `/ebooks/search/` HTML 入口只返回搜索表单，
  不再返回原 `.booklink` 结果。插件搜索改用 Gutenberg 官方 `/ebooks/search.opds/` Atom feed，
  只接受 `/ebooks/<id>.opds` 书目条目并按 feed 的 `rel=next` 判断下一页。
- 新增无网络 OPDS 夹具回归，固定主题条目过滤、作者、详情、章节与 EPUB 获取提案。
- 宿主 User-Agent 更新为带当前版本和项目仓库联系地址的标识，满足源站识别要求；
  既有每域最少 1 秒限速、固定 DNS、禁止重定向和 SSRF 防护保持不变。
- 插件版本升为 `0.1.1` 并重建跟踪 zip。新仓库候选由 `lnr-plugin-2026-01`
  正式私钥签署，只用公钥独立验收通过。
- 新候选位于 `E:\lightnovel-reader-release-staging\v0.1.1-plugin-repository`；
  五文件统一候选位于 `E:\lightnovel-reader-release-staging\v0.3.1-release-rc2`，
  不包含私钥、密码或 unsigned 索引。updater 三个资产复制前后 SHA-256 一致。

验证：

- `cargo test -p reader parses_gutenberg_opds_fixture_without_network -- --nocapture`：通过。
- `cargo test -p reader runs_gutenberg_search_book_chapter_acquire_flow -- --ignored --nocapture`：
  在允许公网访问的环境通过，正式 `gutenberg` 身份成功完成全链路。
- `cargo test -p reader runs_gutenberg_search_book_chapter_acquire_flow -- --ignored --nocapture`：
  在允许公网访问的环境通过；OPDS 返回 25 个 entry，并成功提出
  `https://www.gutenberg.org/ebooks/11.epub3.images`。
- `prepare:plugin-repository-release`：通过；`gutenberg-test@0.1.1` 包 SHA-256 为
  `5ccb02b011143bc685ea9aa1a297c00a2dedb019c4b970aa80bc6c694e0bd2a7`。
- `verify:plugin-repository-release`：新插件候选与统一 RC2 均通过，keyId 为
  `lnr-plugin-2026-01`。
- `cargo test --workspace`：通过（Tauri 8 passed / 1 个公网测试 ignored，reading-core 149 passed）。
- `npm.cmd run check:project`、`npm.cmd run check:release-trust` 与 `npm.cmd run build`：通过。

未验证 / 下一步：

- 统一 RC2 尚未上传 GitHub；旧版本真实检查、下载、安装与重启仍未验证。
- 公开前仍需决定 `gutenberg-test` 是保留为预发布测试资产，还是重命名并调整文案后提升为正式来源。

## 2026-08-04：将 Gutenberg 插件提升为正式来源并组装 RC3

变更：

- 首次公开前将目录从 `examples/gutenberg-test` 重命名为 `examples/gutenberg`，
  manifest id 改为 `gutenberg`、显示名改为 `Project Gutenberg`，正式首版定为 `0.1.0`。
- 描述和合规备注从 E2E 测试文案改为用户可见的公共领域书籍搜索、预览与本地 EPUB 获取说明。
- 运行时夹具、SDK README 与插件契约文档已同步新路径和稳定 id。
- 重建 `gutenberg.zip`，从包内真实 manifest 生成候选，由 `lnr-plugin-2026-01`
  正式私钥签署并只用公钥独立验收。
- 新插件候选位于 `E:\lightnovel-reader-release-staging\v0.1.0-gutenberg`；
  与既有 updater 组装后的五文件统一候选位于
  `E:\lightnovel-reader-release-staging\v0.3.1-release-rc3`，不包含私钥、密码或 unsigned 索引。

已验证：

- `git fetch --all --prune`：远端 `origin/main` 仍为 `a03103c`，无需合并新提交。
- `cargo test -p reader parses_gutenberg_opds_fixture_without_network -- --nocapture`：通过。
- 包内只有 `manifest.json` 与 `plugin.js`，身份为 `gutenberg@0.1.0`。
- 插件包 SHA-256 为 `76f715e85e6360c9a8e0f7ec5bfe5fdaaed26b74221388d4da0d4fc074b0f692`；
  独立候选与统一 RC3 的 Ed25519 验收均通过。
- RC3 的 updater 三个文件与已验收源产物 SHA-256 逐一一致；
  `latest.json` 与 `repository.json` 均指向 `v0.3.1` GitHub Release 的最终资产名。
- `cargo test --workspace`：通过（Tauri 8 passed / 1 个公网测试 ignored，reading-core 149 passed）。
- `npm.cmd run check:project`、`npm.cmd run check:release-trust` 与 `npm.cmd run build`：通过。

待验证 / 下一步：

- `codex/v0.7-release-hardening` 已推送，GitHub PR #45 已创建；GitHub 显示与 `main`
  无冲突且可自动合并。PR 尚未合并，GitHub Release 尚未创建；旧版本真实在线更新仍未验证。

## 2026-08-04：修正 GitHub updater 资产名并创建 v0.3.1 草稿

事实与修正：

- PR #45 已合并到 `main`，远端合并提交为 `479fbd8`。
- 创建首个 `v0.3.1` 草稿 Release 时，GitHub 将带空格的 NSIS 资产名
  `LightNovel Reader_0.3.1_x64-setup.exe` 规范化为点号名，与 RC3 `latest.json` URL 不一致。
- 草稿未公开，因此没有用户影响。`prepare-updater-release.mjs` 现会在输出文件和 URL 中预先把空格换为点号，
  并支持经安全名校验的 `--asset-name`。
- 新增 `test-prepare-updater-release.mjs` 与 npm 入口，固定安装器、`.sig` 和 `latest.json` URL 的同名规则。
- 修正后的统一候选为 `E:\lightnovel-reader-release-staging\v0.3.1-release-rc5`；
  草稿中 RC3 资产已全部替换为 RC5，草稿仍未公开。

验证：

- `npm.cmd run test:prepare-updater-release`：通过。
- 修正后的 `prepare:updater-release` 直接从原 Tauri NSIS 产物成功生成点号资产名；
  `latest.json` URL 为 `.../LightNovel.Reader_0.3.1_x64-setup.exe`，内嵌签名与 `.sig` 一致。
- RC5 `verify:plugin-repository-release`：通过，仍使用 `lnr-plugin-2026-01`。
- GitHub 草稿为 `isDraft=true`、`targetCommitish=main`；五个资产均为 `uploaded`，
  名称、大小和 SHA-256 与 RC5 本地文件逐项一致。
- `npm.cmd run check:project`、`npm.cmd run check:release-trust` 与 `npm.cmd run build`：通过。
- `cargo test --workspace`：通过（Tauri 8 passed / 1 个公网测试 ignored，reading-core 149 passed）。

待验证 / 下一步：

- 草稿尚未公开；需人工最终确认后发布。
- 发布后从旧版本验证检查、下载、安装与重启的真实更新链。

## 2026-08-18：补齐 AGPL-3.0-only 许可证与发布门

完成：

- 从 SPDX 官方 license-list-data 取得未修改的 `AGPL-3.0-only` 标准正文并加入根目录 `LICENSE`；
  本机文件 SHA-256 与官方原文一致。
- `package.json`、`package-lock.json`、`src-tauri`、`reading-core` 与 `sync-server` 统一声明
  `AGPL-3.0-only`；Cargo 包补充项目仓库地址，Tauri crate 作者占位改为项目贡献者。
- 新增 `scripts/check-open-source-license.mjs`：校验标准许可证正文哈希、npm 元数据和三个 Cargo manifest；
  新增 `test-open-source-license.mjs`，覆盖完整配置、正文被修改及包元数据漂移。
- `check:license` 已接入 `check:project`、生产构建、beta/Web 安装器 pre-hook 与正式 Tauri 分发入口。
- README、发布单一入口、PROJECT_MEMORY 和 NEXT_ACTIONS 同步当前信任根、Gutenberg/NSIS 验收、版本轴、
  GitHub 点号资产名和真实在线更新边界。

验证：

- `npm.cmd run check:project`：通过，含架构、记忆、协议、WASM 与许可证检查。
- `npm.cmd run build`：通过，TypeScript 与 Vite/PWA 生产构建成功。
- `npm.cmd run test:license`：通过。
- `npm.cmd run check:release-trust`、`npm.cmd run test:release-trust`、
  `npm.cmd run test:prepare-updater-release`：通过。
- `cargo metadata --no-deps --format-version 1`：通过。
- `cargo test --workspace`：通过（Tauri 8 passed / 1 个真实公网测试 ignored，reading-core 149 passed）。
- `cargo test -p reading-core --features quickjs`：149 passed。
- `git diff --check`：通过；仅报告仓库既有 Windows LF/CRLF 转换提示。

未验证 / 下一步：

- 本轮未提交或推送；草稿 Release 目标仍需先包含 updater 点号资产名修复和许可证收口。
- 未改变仓库 Private 可见性、未编辑或公开 GitHub 草稿，也未执行发布后的旧版本在线更新。
- 未删除 `.codex/.codex`、`docs/docs`、`public/public`、`scripts/scripts`、`tools/tools` 重复目录；
  需维护者确认它们不是需要保留的本地文件后再清理。

## 2026-08-18：统一 v0.7.0 首发版本并增加 CI 质量门

完成：

- 将 npm、Tauri、`reader`、`reading-core` 与 `sync-server` 统一为 `0.7.0`；Cargo 锁文件同步更新。
- Tauri 壳与插件执行器 User-Agent 改为从 `CARGO_PKG_VERSION` 生成，避免代码中继续固化旧版本号。
- 新增 `check-version-alignment.mjs` / `test-version-alignment.mjs`，覆盖一致、漂移与非法 SemVer；版本门已接入
  项目检查、生产构建、beta/Web 安装器和正式分发入口。
- 新增 Windows GitHub Actions CI，覆盖 npm 项目/发布门、版本/许可/updater 回归、前端生产构建、
  Rust workspace、QuickJS 与 rustfmt 检查。
- 现行 README、发布入口、开发大纲、决策、项目记忆与任务队列均以 v0.7.0 为首个公开版本；
  旧 v0.3.1 RC5 和草稿 Release 仅保留为历史证据，明确禁止通过改名复用签名产物。
- 五组误生成的嵌套重复目录已移到可恢复备份
  `C:\Users\41267\Documents\Codex\2026-08-18\n\work\lightnovel-reader-duplicate-backup-20260818`；
  三个内容未变化的 sync 假修改已清理。随后运行 rustfmt，规范了四个既有 Rust 文件的格式。

验证：

- `npm.cmd run check:project`、`npm.cmd run check:release-trust`、`npm.cmd run build`：通过。
- `npm.cmd run test:version`、`test:license`、`test:release-trust`、`test:prepare-updater-release`：通过。
- `cargo test --workspace --locked`：通过（Tauri 8 passed / 1 个公网测试 ignored，reading-core 149 passed）。
- `cargo test -p reading-core --features quickjs --locked`：149 passed。
- `cargo fmt --all -- --check`、`cargo metadata --locked --no-deps --format-version 1`、`git diff --check`：通过。
- `npm.cmd run tauri -- build --bundles nsis --no-sign`：通过，生成
  `target/release/bundle/nsis/LightNovel Reader_0.7.0_x64-setup.exe`，10,530,935 字节，
  SHA-256 `6FAEF42868ECB118B2CA891860EBADDF1A5FE99BCB28DCA593E920E208A12CDD`。

未验证 / 下一步：

- 本轮 NSIS 明确为无签名构建验证，不是正式 updater；尚未用仓库外私钥生成 v0.7.0 `.sig`、`latest.json`
  和最终五资产，也未做 v0.7.0 安装/启动/卸载及在线更新复验。
- 本地提交 `67aa72d` 与其前置 updater 修复已推送到 `codex/v0.7.0-release-finalize`，并创建 PR #46；
  GitHub 判定 `MERGEABLE / CLEAN`。未修改仓库可见性，也未编辑或公开远端 v0.3.1 草稿 Release。
- push 与 pull_request 两次 Actions 运行都在启动前失败。GitHub 网页注解明确指向账户近期付款失败或
  Actions spending limit 不足；没有 job 被执行。官方 actionlint 1.7.12 对 `ci.yml` 校验通过，排除 YAML 语法问题。

## 2026-08-18：增加统一 v0.7.0 发布候选验收

完成：

- PR #46 已合并到远端 `main`（`504635b`），仓库已从 Private 切换为 Public；本地后续分支从该提交建立。
- 新增 `verify-release-candidate.mjs` / `verify:release-candidate`：从 Tauri 配置确定版本/tag，从
  `plugin_trust.rs` 读取编译内 Ed25519 公钥；核对 GitHub Release URL、点号 NSIS 名、`.sig` 与
  `latest.json` 签名文本、插件包 SHA-256/大小/签名，以及候选目录精确文件集合。
- 新增 `test-verify-release-candidate.mjs` / `test:release-candidate`，并接入 GitHub Actions 发布回归。
- 发布入口、决策、项目记忆和下一步队列同步统一验收命令与资产白名单边界。

验证：

- `npm.cmd run test:release-candidate`：通过，覆盖合法五资产、updater 签名漂移、旧 v0.3.1 tag、
  插件字节篡改和额外 `updater-private.key` 拒绝。
- 对仓库外历史候选 `v0.3.1-release-rc5` 运行统一验收：按预期退出 1，明确拦截旧版本、旧 tag URL
  和旧安装器资产名，证明旧签名产物不能伪装成 v0.7.0 候选。
- `node --check` 两个新增脚本、`npm.cmd run check:project`、`check:release-trust`、`npm.cmd run build`：通过。
- `npm.cmd run test:version`、`test:license`、`test:release-trust`、`test:prepare-updater-release`、
  `test:release-candidate`：通过。
- `cargo test --workspace --locked`：通过（Tauri 8 passed / 1 个公网测试 ignored，reading-core 149 passed）。
- `cargo test -p reading-core --features quickjs --locked`：149 passed。
- `cargo fmt --all -- --check`、官方 actionlint 1.7.12、`git diff --check`：通过。

未验证 / 下一步：

- 本轮提交 `7018990` 已推送并创建 PR #47；push 运行 `32130316608` 与 pull_request 运行
  `32130322027` 均真实执行全部 Windows job 并全绿，证明 Actions 后台锁已解除。Support #4676102
  可在 GitHub 回复后关闭。
- 正式 v0.7.0 签名五资产尚未生成，因此本轮只用临时公私钥和合成资产回归验收逻辑，不声称正式候选已通过。

## 2026-08-20：轮换 updater 密钥并生成 v0.7.0 正式五资产

完成：

- 在仓库外秘密目录定位插件与 updater 两套密钥；两个公钥文件分别与编译内插件 keyring、Tauri updater
  配置精确匹配，未读取或输出私钥正文。
- 旧 updater 私钥两次交互构建均在签名阶段明确报密码错误。确认 GitHub 没有公开 Release/Tag 后，保留旧密钥，
  生成新的带密码 updater 密钥，并将 `tauri.conf.json` 更新为新公钥。
- 用 `lnr-plugin-2026-01` 重新生成、签署并独立验收 v0.7.0 Gutenberg 仓库；用新 updater 密钥完成正式
  NSIS 签名构建。
- 隔离历史 v0.3.1 bundle 后生成 updater 三资产，组装
  `E:\lightnovel-reader-release-staging\v0.7.0-release` 五资产并通过 `verify:release-candidate`。
- 正式 NSIS 静默安装退出码 0，安装版成功启动；静默卸载退出码 0，安装目录已移除。卸载前后
  `reader.db` 与 `library.sqlite` 的数量、长度、SHA-256 完全一致。

验证：

- `npm.cmd run check:project`、`check:release-trust`、`npm.cmd run build`：通过。
- `npm.cmd run test:version`、`test:license`、`test:release-trust`、`test:prepare-updater-release`、
  `test:release-candidate`：通过。
- `cargo test --workspace --locked`：通过（Tauri 8 passed / 1 个公网测试 ignored，reading-core 149 passed）。
- `cargo test -p reading-core --features quickjs --locked`：149 passed。
- `cargo fmt --all -- --check`、官方 actionlint 1.7.12、`git diff --check` 与敏感信息 diff 扫描：通过。

候选摘要：

- `LightNovel.Reader_0.7.0_x64-setup.exe`：10,530,672 字节，SHA-256
  `6BA69BE9FB71E73BFCA9FEA5B1E42DF752B3279177448A4B48EE0419F34021C3`。
- `LightNovel.Reader_0.7.0_x64-setup.exe.sig`：432 字节，SHA-256
  `A69FDEAACB574F25841001B55FBAC0F52D963AD439A38F6B53BD586FF6D270BA`。
- `gutenberg.zip`：1,763 字节，SHA-256
  `76F715E85E6360C9A8E0F7EC5BFE5FDAAED26B74221388D4DA0D4FC074B0F692`。
- `latest.json`：754 字节，SHA-256
  `07A060AE909B66E881B6933B77CC3B5436A830D187EAC6307C657302ECBFFF67`。
- `repository.json`：1,345 字节，SHA-256
  `1495B8F66C17A0F8BDA046C2D7CA832D7BDCEE975471A256DC3C87EAB2DDF89D`。

未验证 / 下一步：

- 新 updater 公钥尚未提交、推送和合并，远端草稿不得在源码目标仍为旧公钥时公开。
- v0.7.0 五资产尚未上传 GitHub Draft Release；远端大小/SHA-256 与公开后的在线更新尚未复验。

## 2026-08-20：创建 v0.7.0 草稿并核对远端五资产

完成：

- 新 updater 公钥与候选记录通过 PR #48 合并到 `main`，合并提交为
  `cb75a53bbd37986775a95a45229196d3a46045ad`；push / pull_request 两条 Windows CI 均全绿。
- 创建新的 `v0.7.0` Draft Release，目标固定为上述合并提交，上传正式候选的五个公开资产；旧 v0.3.1
  草稿未删除，v0.7.0 草稿未公开。
- 通过 GitHub Release API 严格复核：草稿状态为 true、tagName 为 `v0.7.0`、目标提交一致，远端恰好五个
  资产；每个名称、字节数与 GitHub `sha256:` digest 均匹配本机候选。

未验证 / 下一步：

- Draft 资产 URL 当前使用 GitHub 内部 `untagged-*` 路径；只有公开后才会落到清单声明的
  `/releases/download/v0.7.0/`，因此当前不能做真实在线更新结论。
- 发布说明、许可证、目标提交和资产仍需维护者人工审阅并明确同意；本轮未公开 Release、未创建远端 tag。

## 2026-08-21：公开 v0.7.0 并完成公开资产复验

完成：

- 维护者明确同意公开后，将 `v0.7.0` Draft Release 发布为正式、非预发布的 Latest Release：
  https://github.com/haryqs/lightnovel-reader/releases/tag/v0.7.0 。Release 目标与远端 tag 均固定到
  `cb75a53bbd37986775a95a45229196d3a46045ad`，公开资产仍严格为五个。
- 从无需登录的公开 `/releases/download/v0.7.0/` URL 重新下载全部五资产，运行统一发布候选验收并通过；
  五个文件的名称、字节数与 SHA-256 均和仓库外正式候选一致。
- 从 `/releases/latest/download/latest.json` 单独下载 Latest 别名清单，其 SHA-256 为
  `07A060AE909B66E881B6933B77CC3B5436A830D187EAC6307C657302ECBFFF67`，与版本化 `latest.json` 一致。
- 使用公开下载的 NSIS 再次静默安装并实际启动应用，随后静默卸载；安装、启动、卸载退出状态正常，安装目录
  已移除，既有 `reader.db` 与 `library.sqlite` 的数量、长度和 SHA-256 均保持不变。

验证：

- `npm.cmd run verify:release-candidate -- --dir <公开下载目录> --tag v0.7.0`：通过。
- 公开五资产与正式候选逐项 SHA-256：一致；远端 `refs/tags/v0.7.0`：`cb75a53`。
- 公开安装器：静默安装退出码 0，安装版实际启动成功，静默卸载退出码 0，用户数据保留。

边界 / 下一步：

- v0.7.0 是首次公开前轮换的新 updater 信任根下的第一个公开版本，没有更早的公开同信任根客户端可用于
  真正的跨版本更新。因此公开下载与安装链已验证，但自动检查、下载、安装、重启的完整 updater 闭环必须
  在发布 v0.7.1 时从公开 v0.7.0 验证。
- 旧 v0.3.1 草稿继续只作为历史证据保留，不属于公开候选；本轮未删除它。
