# 下一步任务队列

## 📌 交接留言（2026-06-22，v0.7 插件 host API 策略层进行中）

先同步 GitHub：

```powershell
cd E:\workspace\game-cooperative-plan\lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

当前事实：

- 当前工作分支：`codex/plugin-host-api-policy`。
- 已新增 `reading-core::plugin_host`：源插件方法 DTO、搜索/书籍/章节/acquire DTO、`host.http` 请求计划、KV 与 acquire 策略门。
- 运行前策略已覆盖：停用插件不得运行；`browse/resolveUrl/fetchMetadata/acquire` 必须声明对应 capability；`host.http` 必须有 `http` 权限、精确命中 manifest 域名、超时 1..=60000ms、忽略 User-Agent/Referer/Cookie/Authorization/Host/Origin 等保留头。
- `host.kv` 必须有 `kv` 权限，key 最大 128 字符，value 最大 64 KiB。
- `acquire` 仍只是插件提案：`metadataOnly` 不下载；`download/cacheForReading` 第一版只放行 `public_domain` 与 `open_license`，`official_free` 在 ToS/限速门控落地前继续 metadata + 官方外链。
- 当前仍不执行插件 JS，不新增桥接协议消息，不引入 QuickJS。

本轮验证待收工填写：

- `node scripts/check-arch.mjs`
- `node scripts/check-dev-memory.mjs`
- `node scripts/check-protocol-freeze.mjs`
- `npm.cmd run build`
- `cargo test --workspace`
- `git diff --check`

下一步优先级：

1. 后续 v0.7 运行时落地时，QuickJS/JavaScriptCore host 必须复用 `plugin_host` 策略函数，不能绕过到壳侧直接发 HTTP/写 KV。
2. 设计官方插件仓库索引/签名草案，仍保持用户自装插件与官方白名单插件视觉区分。
3. 如要放行某个 `official_free` 源的正文获取，先补源站 ToS 记录、限速策略、用户确认与单源测试。

## 📌 交接留言（2026-06-22，v0.7 插件启用状态骨架完成）

先同步 GitHub：

```powershell
cd E:\workspace\game-cooperative-plan\lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

当前事实：

- 当前工作分支：`codex/plugin-enable-state`。
- 已安装插件元数据新增 `enabled` 字段；旧安装记录缺字段时默认启用。
- 已新增 `plugin.setEnabled(pluginId, enabled)` 协议能力、Tauri command 与书库 UI 按钮。
- 停用只更新 `install.json`；不删除插件文件，不执行插件 JS。
- `plugin-sdk/examples/*.zip` 已加入 `.gitignore`，用于忽略本地测试打包产物。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-protocol-freeze.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 104 passed）。
- `git diff --check` 通过（仅 Windows 换行提示）。

下一步优先级：

1. 继续 v0.7 host API 纯 Rust 输入输出结构。
2. 后续运行时落地时必须跳过 `enabled=false` 的插件。
3. 仍不接正文抓取源；插件执行前先把 host API 权限门控与域名白名单测试补齐。

## 📌 交接留言（2026-06-22，v0.7 插件安装 UI/权限确认完成）

先同步 GitHub：

```powershell
cd E:\workspace\game-cooperative-plan\lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

当前事实：

- 已新增 `reading-core::plugin_store`：预览插件 zip、安装写入 app data 插件目录、列出已安装插件。
- 已新增协议能力：`plugin.selectPackagePath`、`plugin.inspectPackage`、`plugin.installPackage`、`plugin.listInstalled`。
- 桌面壳引入官方 Tauri dialog 插件，用原生文件选择器取得 zip 路径；协议消息不传插件 zip 字节。
- 书库已有“源插件（v0.7 预览）”面板：展示 manifest、域名、权限、能力、授权类型、warnings；`user-declared` 必须勾选确认后才能安装。
- 当前仍不引入 QuickJS、不执行插件 JS、不把正文/章节抓取源塞回内核连接器。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-protocol-freeze.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 102 passed）。
- `npm.cmd run tauri -- build --debug --no-bundle` 通过。
- `git diff --check` 通过（仅 Windows 换行提示）。

下一步优先级：

1. 后续 v0.7 下一块建议做 host API 纯 Rust 输入输出结构，或插件启用/禁用状态；仍不要执行第三方 JS。
2. 后续若做插件运行时，先实现 host API 权限门控与域名白名单测试，再接 QuickJS。
3. 继续保持边界：官方内核连接器不接正文/章节抓取源，插件也必须经过安装确认与合规门控。

## 📌 交接留言（2026-06-22，v0.7 插件 zip 安装包读取骨架完成）

先同步 GitHub：

```powershell
cd E:\workspace\game-cooperative-plan\lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

当前事实：

- `main` 已包含 PR #37：`reading-core::plugin_manifest` manifest/权限/合规策略骨架。
- v0.7 插件安装地基继续推进：已新增 `reading-core::plugin_package`，从 zip 字节读取唯一 `manifest.json`，复用 manifest 校验，确认入口 JS 存在并返回入口文本。
- 插件包规则：支持根目录或单层目录 zip；只允许一个 manifest；入口脚本必须与 manifest 同目录，且是单个 `.js` 文件名；安装前只读取/校验，不执行插件代码。
- 本轮仍不新增桥接消息、不引入 QuickJS、不执行插件、不接正文抓取源。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-protocol-freeze.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 99 passed）。
- `git diff --check` 通过（仅 Windows 换行提示）。

下一步优先级：

1. 做插件安装 UI/权限确认：选择 zip、展示 manifest 能力/域名/合规声明、要求用户确认后才写入本地插件存储。
2. 或先补 host API 纯 Rust 接口草案：围绕 `search` / `browse` / `fetchMetadata` / `resolveUrl` / `acquire` 做输入输出结构，不执行 JS。
3. 继续保持边界：正文/章节抓取类来源不要塞回内核连接器；合法开放资源与用户自有资源才允许站内获取/阅读。

## 📌 交接留言（2026-06-22，v0.7 插件 manifest 策略骨架进行中）

先同步 GitHub：

```powershell
cd E:\workspace\game-cooperative-plan\lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

当前事实：

- `main` 已包含协议 `1.0-rc.1` 冻结候选与 `scripts/check-protocol-freeze.mjs` 守门脚本。
- 当前分支 `codex/plugin-manifest-policy` 从最新 `main` 创建，开始推进 v0.7 插件运行时的宿主侧策略骨架。
- 已新增 `reading-core::plugin_manifest`：解析/校验 manifest、精确域名白名单、权限/能力去重、`user-declared` 明示确认、`official-free + acquire` ToS warning。
- 已同步 `plugin-sdk/manifest.schema.json` 的 `capabilities` 字段，并补充 `source-plugin.d.ts` 可选能力方法。
- 本轮不新增桥接消息、不引入 QuickJS、不执行插件、不接正文抓取源；只打宿主侧安装/权限/合规门控地基。
- 合法边界不变：Bangumi / なろう 只做 metadata + 外链；`library.acquireRemote` 只允许青空公共版权条目；OPDS 下载只允许 `open_license`。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-protocol-freeze.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 92 passed）。
- `node -e "JSON.parse(...)"` 校验 plugin-sdk schema 与示例 manifest 通过。
- `git diff --check` 通过；仅有 Windows 换行提示。
- `cargo fmt --check` 未作为本轮门槛：当前仓库既有多处 Rust 文件会被 rustfmt 改写，已仅对本轮新增的 `plugin_manifest.rs` 做局部 `rustfmt`，避免无关格式噪声。

下一步优先级：

1. 提交、推送 `codex/plugin-manifest-policy`，开 PR 后合入主线。
2. 后续 v0.7 下一块建议做插件安装包读取/权限确认 UI 草图或 host API 纯 Rust 接口；仍不要把正文/章节类来源塞回内核连接器。
3. 分发线仍需目标机器便携包下载/解压/启动抽检；安装版仍需 NSIS 安装/卸载并确认不删除 `%APPDATA%` 用户书库数据。

## 📌 交接留言（2026-06-22，协议冻结自动守门进行中）

先同步 GitHub：

```powershell
cd E:\workspace\game-cooperative-plan\lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

当前事实：

- `main` 已包含协议 `1.0-rc.1` 冻结候选与结构化 `BridgeError` 收口。
- 本轮分支 `codex/protocol-freeze-guard` 新增 `scripts/check-protocol-freeze.mjs`，用于守住协议冻结的三边一致性：TS 协议类型、Rust 壳侧错误码、协议文档。
- `package.json` 已新增 `check:protocol`，并把协议冻结检查接入 `check:project` 与 `npm.cmd run build`。
- 冻结纪律不变：后续默认只允许新增消息/新增可选字段，不随意改名、删字段或改语义；若新增错误码，必须同步 TS 类型、Rust 构造器/壳侧实现与文档 8。
- 版权边界不变：Bangumi / なろう 只做 metadata + 外链；`library.acquireRemote` 只允许青空公共版权条目；OPDS 下载只允许 `open_license`。

已验证：

- `node scripts/check-protocol-freeze.mjs` 通过。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run check:project` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过；仅有 Windows 换行提示。

下一步优先级：

1. 提交并推送 `codex/protocol-freeze-guard`，开 PR 后合入主线。
2. 合并后继续协议冻结候选 review。
3. 分发线继续目标机器便携包下载/解压/启动抽检；安装版继续 NSIS 安装/卸载并确认不删除 `%APPDATA%` 用户书库数据。
4. 功能线下一段建议进入 v0.7 插件运行时 / ToS 门控设计，不要把正文/章节类来源塞回内核连接器。

## 📌 交接留言（2026-06-22，协议 1.0-rc.1 冻结候选进行中）

先同步 GitHub：

```powershell
cd E:\workspace\game-cooperative-plan\lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

当前事实：

- PR #33（`codex/protocol-freeze-audit`）已合并进 `main`，预取语义与资源通道审计已进入主线。
- 当前工作分支是 `codex/protocol-error-freeze-audit`，从合并 PR #33 后的最新 `main` 创建。
- 本轮把 `PROTOCOL_VERSION` 从 `0.1` 推进到 `1.0-rc.1`，协议文档标题同步为“桥接协议 v1.0-rc.1”；文件名暂沿用 `8_桥接协议_v0.1.md` 以保持历史链接稳定。
- Tauri command 面此前已全部返回 `BridgeError`；本轮补齐 `shell.openExternal` / `shell.openPathExternal` 的结构化错误包装，并让无原生 bridge 兜底也抛同形态对象。
- 新增错误码 `platformError`，用于系统浏览器/外部阅读器打开失败、无桌面壳等平台能力失败；空 URL/path 仍走 `invalidArgument`。
- 版权边界不变：Bangumi / なろう 只做 metadata + 外链，不抓正文；`library.acquireRemote` 仍只允许青空公共版权条目，OPDS acquire 仍强制 `open_license`。

已验证：

- `npm.cmd run build` 通过。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过；仅有 Windows 换行提示。

下一步优先级：

1. 提交并推送 `codex/protocol-error-freeze-audit`，打开 PR。
2. PR 合并后，如果没有审计阻塞，可把协议冻结候选进入 review：只允许新增消息/新增可选字段，不再随意改名、删字段或改语义。
3. 分发线继续用当前便携包候选做目标机器下载/解压/启动抽检；安装版另跑 NSIS 安装/卸载，并确认卸载不删除 `%APPDATA%` 用户书库数据。
4. 功能线下一段建议进入 v0.7 插件运行时 / ToS 门控设计，不要把正文/章节类来源塞回内核连接器。

## 📌 交接留言（2026-06-22，协议冻结审计：预取/资源通道已收口）

当前分支：

- `codex/protocol-freeze-audit`，从最新 `main`（PR #32 已合并、便携包候选已重验后）创建。

本轮已完成：

- 审计批量/预取语义：
  - 冻结前不新增 `chapter.prefetch` / `chapter.getBatch`。
  - `chapter.get(href)` 保持唯一章节 HTML 获取消息，允许 reader-engine 用它做前台加载与后台预取。
  - 当前 `ReaderCore.preloadAroundChapter` 已有有界预取：前一章、后一章、后两章；前端有
    `chapterInflight` 去重与 `maxCachedChapters=10`。
  - core/Tauri 侧有当前书章节内存缓存与持久化 parse cache。
- 审计资源通道边界：
  - 仅保留 `book.open(data)` 与 `library.importBytes(data)` 两个移动/沙盒兜底大字节例外。
  - 桌面导入/开书/OPDS/青空获取不在 JSON 消息里搬运整本书；使用路径、id、HTTP 壳侧下载与对象仓库引用。
  - 书内图片走 `reader-img` URL scheme；本地封面/缩略图走 `resource.url`；远程封面保持来源 http(s) URL。
- `docs/resource-library-plan/8_桥接协议_v0.1.md` 已新增两个审计小节，并把冻结前检查清单第 2 / 第 4
  项标记完成。
- `docs/dev-memory/DECISIONS.md` 已新增对应决策。

下一步优先级：

1. 提交并推送 `codex/protocol-freeze-audit`，开 PR。
2. 后续继续协议冻结：结构化错误码范围最终核对、确认是否将 `PROTOCOL_VERSION` 从 `0.1` 进入冻结候选。
3. 若准备发便携测试包：仍可使用当前 `dist-beta/lightnovel-reader-v0.1.0-release-windows-x64.zip` 候选，
   在目标机器做下载/解压/启动抽检。

## 📌 交接留言（2026-06-22，PR #32 已合并 + 便携包候选已重验）

先同步 GitHub：

```powershell
cd E:\workspace\game-cooperative-plan\lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

当前事实：

- PR #32（`codex/remaining-bridge-errors`）已合并进 `main`，远端分支已删除。
- `book.*`、`chapter.get`、`annotation.*`、`reading.*` 已与 `opds.*` / `library.*` 一样迁移到结构化
  `BridgeError { code, message, details? }`。
- 当前 release 便携包候选已重新生成：
  `dist-beta/lightnovel-reader-v0.1.0-release-windows-x64.zip`。
- 已把 zip 解压到 `dist-beta/extract-check-release` 并用解压后的 `reader.exe` 跑过真实 Tauri 启动冒烟。

已验证：

- PR #32 合并前：`cargo check --workspace`、`node scripts/check-arch.mjs`、
  `node --check scripts/tauri-opds-smoke.mjs`、`node scripts/check-dev-memory.mjs`、
  `npm.cmd run build`、`cargo test --workspace`、`git diff --check` 均通过。
- PR #32 合并后：`npm.cmd run package:beta` 通过。
- PR #32 合并后：解压后的 release `reader.exe` 启动冒烟通过：
  `npm.cmd run smoke:tauri -- --tauri-driver C:\Users\41267\.cargo\bin\tauri-driver.exe --native-driver C:\Users\41267\AppData\Local\lightnovel-reader-tools\msedgedriver\149.0.4022.69\msedgedriver.exe --application E:\workspace\game-cooperative-plan\lightnovel-reader\dist-beta\extract-check-release\reader.exe`

下一步优先级：

1. 若准备发便携测试包：把当前 zip 作为候选，在目标机器做下载/解压/启动抽检；需要更稳时再补跑
   `smoke:p0` / `smoke:p1` / `smoke:remote-link` / `smoke:opds`。
2. 若继续开发：做协议冻结审计，优先核对批量/预取语义、资源通道边界、后续新增命令默认使用
   `BridgeError`。
3. 若做安装版：重跑 NSIS 安装 / 卸载，并确认卸载不删除 `%APPDATA%` 用户书库数据。

## 📌 交接留言（2026-06-22，剩余阅读/标注命令 BridgeError 迁移）

当前分支：

- `codex/remaining-bridge-errors`，从 `main` 快进到 GitHub 最新 `0b027bf` 后创建。

本轮已完成：

- 已同步 GitHub：`git fetch --all --prune` 拉到 `origin/main 0b027bf`，`git pull --ff-only`
  快进本地 `main`；远端 `codex/source-record-panel` 已删除，PR #22 已合并事实不变。
- `src-tauri/src/lib.rs`：剩余阅读/标注相关命令已迁移到结构化 `BridgeError`：
  - `book.open` / `book.openPath` / `book.close`
  - `chapter.get`
  - `annotation.save` / `annotation.list` / `annotation.delete`
  - `reading.saveProgress` / `reading.getProgress`
- 错误分类沿用既有 7 个 code，不新增 code：
  - 空参数：`invalidArgument`
  - 未开书/找不到目标：`notFound`
  - EPUB/章节解析：`parseError`
  - Mutex/SQLite/文件读取等本地状态问题：`storageError`
- `src/platform/protocol.ts` 与 `docs/resource-library-plan/8_桥接协议_v0.1.md` 已同步结构化错误码覆盖范围。
- 合规/获取边界不变：Bangumi / なろう 仍只做 metadata + 外链；青空 `public_domain` 与 OPDS
  `open_license` 的正文获取硬门不变。

已验证：

- `cargo check --workspace` 通过。
- `node scripts/check-arch.mjs` 通过。
- `node --check scripts/tauri-opds-smoke.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过；仅有 Windows 换行提示。

下一步优先级：

1. 提交并推送 `codex/remaining-bridge-errors`，开 PR。
2. 继续协议冻结审计：批量/预取语义、资源通道核对、后续新增命令默认使用 `BridgeError`。
3. 若准备分发，重跑 `npm.cmd run package:beta` 并做解压/启动检查。

## 📌 交接留言（2026-06-21，OPDS acquisition URL 持久化已落地）

当前分支：

- `codex/opds-acquisition-url`，从已合并 PR #30 后的最新 `main` 创建。

本轮已完成：

- `source_record` 新增迁移 v6：`acquisition_url TEXT`，用于保存合法开放正文获取链接；`remote_url` 继续只表示官方/来源页面外链。
- `RemoteEntry` / `connectors::ingest` / `LibraryBook` / `LibrarySourceRecord` / `RemoteAcquisition` 已贯通 `acquisitionUrl`。
- OPDS feed 入库前会解析相对链接，开放授权条目的 `acquisitionUrl` 会随来源记录持久化。
- `opds.downloadEpub(editionId, acquisitionUrl?)` 第二参数改为可选；未传时从库内 `source_record.acquisition_url` 回读，并继续强制 `rightsStatus=open_license`。
- 书架远程 OPDS `open_license` 条目现在可直接显示“获取”动作，获取后按阅读偏好打开。
- `scripts/tauri-opds-smoke.mjs` 已扩展：先加入书架，确认 `acquisitionUrl` 持久化，再从书架卡片获取并阅读。
- README、桥接协议文档、schema 草案已同步。

已验证：

- `node --check scripts/tauri-opds-smoke.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `npm.cmd run tauri -- build --debug --no-bundle` 通过。
- `npm.cmd run smoke:opds -- --tauri-driver C:\Users\Administrator\.cargo\bin\tauri-driver.exe --native-driver C:\Users\Administrator\AppData\Local\lightnovel-reader-tools\msedgedriver\149.0.4022.62\msedgedriver.exe` 通过。
  - source: `https://www.gutenberg.org/ebooks/search.opds/?query=austen`
  - remote: `Pride and Prejudice`
  - acquisitionUrl: `https://www.gutenberg.org/ebooks/1342.epub.noimages`
  - reader UI: `libraryHidden=true`, `readingActive=true`, `statusbarHidden=false`

下一步优先级：

1. 继续迁移 `book.*` / `annotation.*` / `reading.*` 到结构化 `BridgeError`。
2. 若准备分发，重跑 `npm.cmd run package:beta` 并做解压/启动检查。

## 📌 交接留言（2026-06-21，OPDS 获取并阅读冒烟已通过）

当前分支：

- `codex/opds-acquire-smoke`，从已合并 PR #29 后的最新 `main` 创建。

本轮已完成：

- 更新 `scripts/tauri-opds-smoke.mjs`：兼容新的“获取并阅读”按钮，同时保留旧 `EPUB` 文本兼容。
- 冒烟新增阅读态断言：获取 OPDS EPUB 后必须进入 `reading-active`，书库层必须隐藏，状态栏可见，标题匹配已打开作品。
- 修复真实冒烟发现的问题：OPDS 获取后先打开阅读器、再 `refreshLibraryBooks()` 会把书库层重新显示出来；现在先刷新书库，再按偏好打开获取后的本地 asset。

已验证：

- `node --check scripts/tauri-opds-smoke.mjs` 通过。
- `npm.cmd run tauri -- build --debug --no-bundle` 通过。
- `npm.cmd run smoke:opds -- --tauri-driver C:\Users\Administrator\.cargo\bin\tauri-driver.exe --native-driver C:\Users\Administrator\AppData\Local\lightnovel-reader-tools\msedgedriver\149.0.4022.62\msedgedriver.exe` 通过。
  - source: `https://www.gutenberg.org/ebooks/search.opds/?query=austen`
  - downloaded: `Pride and Prejudice`
  - reader UI: `libraryHidden=true`, `readingActive=true`, `statusbarHidden=false`

下一步优先级：

1. 若继续 OPDS 平台化：设计并落地 acquisition URL 持久化，让书架远程 OPDS 条目也能直接“获取并阅读”（不要复用 `remoteUrl`）。
2. 若继续协议内功：迁移 `book.*` / `annotation.*` / `reading.*` 到结构化 `BridgeError`。
3. 若准备分发：重跑 `npm.cmd run package:beta` 并做解压/启动检查。

## 📌 交接留言（2026-06-21，统一合法资源获取后阅读第一步）

当前分支：

- `codex/unified-acquire-open`，从已合并 PR #28 后的最新 `main` 创建。

本轮已完成：

- 青空 `public_domain` 远程条目经 `library.acquireRemote` 获取后，不再固定内置打开，而是走统一的 `openAcquiredLibraryBook`：默认阅读方式为 `外部` 且有 `filePath` 时交给系统默认本地阅读器，否则进入内置阅读器。
- OPDS feed 面板的开放授权 EPUB 按钮从“下载 EPUB”改为“获取并阅读”。
- OPDS `open_license` 条目点击“获取并阅读”后，会先 `opds.ingestEntries` 落库拿 `editionId`，再 `opds.downloadEpub` 下载并 attach 成本地 asset，完成后复用同一套 `openAcquiredLibraryBook` 打开。
- 合规边界不变：OPDS 下载命令层仍强制 `rightsStatus=open_license`；青空正文获取仍只允许 `public_domain`。

已验证：

- `npm.cmd run build` 通过（含 `check-arch`、`tsc`、`vite build`）。
- `node scripts/check-dev-memory.mjs` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过（仅 Windows 换行提示）。

下一步优先级：

1. 做真实 Tauri/OPDS 冒烟：添加开放授权 OPDS 源 → 浏览 feed → 点击“获取并阅读” → 确认下载、入库、按偏好打开。
2. 若要从书架远程 OPDS 条目直接获取，需要给书库模型持久化 acquisition URL（不要滥用 `remoteUrl`，它仍应表示官方/来源外链）。
3. 若继续协议内功，迁移 `book.*` / `annotation.*` / `reading.*` 到结构化 `BridgeError`。

## 📌 交接留言（2026-06-21，阅读方式偏好已完成）

当前分支：

- `codex/reading-preference`，从已合并 PR #27 后的最新 `main` 创建。

本轮已完成：

- 书库标题栏新增默认阅读方式选择：`自动 / 内置 / 浏览器 / 外部`。
- 偏好写入 `localStorage` 的 `reader.libraryReadPreference`，与主题、版式、单双页同类，暂不引入 DB schema。
- 书架卡片仍展示全部可用动作；主按钮高亮和卡片点击会按偏好选择动作，不可用时自动回退。
- 合规边界不变：`public_domain` 可走获取后内置阅读；`official_free` / `official_purchase` / `unknown` 不抓正文，只跳官方入口。

已验证：

- `npm.cmd run build` 通过（含 `check-arch`、`tsc`、`vite build`）。
- `node scripts/check-dev-memory.mjs` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过（仅 Windows 换行提示）。

下一步优先级：

1. 若继续平台化体验，优先做 OPDS `open_license` 与青空 `public_domain` 的统一 acquire/open 动作入口。
2. 若继续协议内功，迁移 `book.*` / `annotation.*` / `reading.*` 到结构化 `BridgeError`，为协议冻结审计做准备。

## 📌 交接留言（2026-06-21，阅读方式选择第一版已完成）

当前分支：

- `codex/reading-action-model`，已合并 PR #26（平台定位升级）后的最新 `main` 上创建。

本轮已完成：

- 书架卡片新增第一版阅读方式动作模型：
  - 本地 / cached asset：`内置` 打开内置阅读器；有 `filePath` 时可点 `外部` 交给系统默认本地阅读器。
  - 远程 metadata-only：有 `remoteUrl` 时点 `浏览器` 跳官方入口。
  - `public_domain` 远程条目：点 `获取` 走 `library.acquireRemote`，获取后用内置阅读器打开。
  - `official_free` / `official_purchase` / `unknown` 不进入正文抓取，仍只跳官方入口。
- 协议新增 `shell.openPathExternal`，Tauri 适配层用 `@tauri-apps/plugin-opener` 的 `openPath` 实现；前端业务仍只通过 `src/platform/ReaderBridge` 调用。

已验证：

- `npm.cmd run build` 通过（含 `check-arch`、`tsc`、`vite build`）。
- `node scripts/check-dev-memory.mjs` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过（仅 Windows 换行提示）。

## 📌 交接留言（2026-06-21，定位升级为轻小说平台）

当前分支：

- `codex/platform-positioning`，从已合并 PR #25 后的最新 `main` 创建。

本轮产品定位：

- `lightnovel-reader` 不再只按“轻小说阅读器”定义，而是**本地优先轻小说平台**。
- 阅读器是核心模块；平台还包括发现、索引、收藏、整理、合法获取入口、来源记录、阅读方式选择和未来插件生态。
- 合法开放资源（公共版权、开放授权、用户自有资源、经 ToS/授权确认可获取的官方免费资源）可以进入站内获取/缓存/阅读流程。
- 商业、受保护或未知授权正文仍只保存 metadata 与官方入口，不自动抓取、不缓存、不镜像。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `git diff --check` 通过；仅有 Windows 换行提示。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。

下一步优先级：

1. 做“阅读方式选择”第一版动作模型与 UI：
   - 本地/cached asset：内置阅读器打开、外部本地阅读器打开。
   - 远程 metadata-only：浏览器打开官方入口。
   - public_domain/open_license：获取/缓存后用内置阅读器打开。
   - official_free：默认浏览器打开；只有来源 ToS/API 明确允许时才进入 acquire。
2. 若新增外部阅读器打开能力，必须经 `src/platform/` 暴露平台命令；不要让前端业务代码直接 import Tauri。
3. 后续补协议/DTO 时考虑动作枚举：`openInBrowser`、`openInBuiltinReader`、`openInExternalReader`、`acquireThenOpen`。

## 📌 交接留言（2026-06-21，library.* 结构化错误码进行中）

先同步 GitHub：

```powershell
cd E:\workspace\game-cooperative-plan\lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

当前分支：

- `codex/library-bridge-errors`，从已合并 PR #24 后的最新 `main` 创建。

当前事实：

- PR #24 已合并到 `main`，OPDS 第一批结构化错误码已进入主线。
- 本轮继续迁移 `library.*`：
  `listCalibre / import / importBytes / list / search / listSourceRecords / searchRemote /
  searchRemoteSource / acquireRemote / linkRemoteToLocal / open / touchLastRead`
  已改为返回 `BridgeError`。
- 远程搜索与正文获取按 `networkError`、`httpStatus`、`parseError`、`storageError`、`notFound`、
  `forbidden` 分类；前端书库错误展示已走 `formatError(e)`。
- 根 `README.md` 已更新为当前项目入口，覆盖能力、架构、合规边界、开发/验证/打包命令。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo check --workspace` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过；仅有 Windows 换行提示。

下一步优先级：

1. 提交并推送 `codex/library-bridge-errors`，开 PR。
2. 后续继续迁移 `book.*`、`annotation.*`、`reading.*` 到 `BridgeError`，最后做协议冻结审计。

## 📌 交接留言（2026-06-21，结构化错误码前端消费补齐）

先同步 GitHub：

```powershell
cd E:\workspace\game-cooperative-plan\lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

当前分支：

- `codex/structured-opds-errors`，以最新 `main` 为祖先；`main` 已包含 PR #23（OPDS URL 粘贴识别 +
  `npm.cmd run smoke:opds` 真实 Tauri 联网冒烟脚本）。

当前事实：

- 2026-06-19 第一批已经把 v0.6 OPDS 网络/存储面命令迁到结构化 `BridgeError`
  （`invalidArgument / storageError / parseError / networkError / httpStatus / notFound / forbidden`）。
- 本轮补齐前端真实消费：`src/main.ts` 引入 `isBridgeError`，`formatError(err)` 识别
  `BridgeError` 后显示 `message` + `code` + 可选 `details`。
- OPDS 源列表、添加/移除源、浏览/搜索 feed、加入书架、下载 EPUB、批量加入书架的错误展示都已走
  `formatError(e)`；这使 `src/platform/protocol.ts` 的 `isBridgeError` 链路不再只是类型守卫。
- 协议文档 8 已同步说明当前 UI 消费方式；`tauri.ts` 仍无需改动。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `git diff --check` 通过；仅有 Windows 换行提示。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。

下一步优先级：

1. 提交并推送 `codex/structured-opds-errors`，打开/更新 PR。
2. 若继续同一方向，按价值迁移其余命令（`library.*` / `annotation.*` / `reading.*` / `book.*`）
   到 `BridgeError`，迁移后再定稿协议冻结的错误码范围。
3. 若做体验增强，可基于 `BridgeError.code` 给网络错误、HTTP 状态、版权拒绝分别做更具体的 UI 提示或重试入口。

## 📌 交接留言（2026-06-19，结构化错误码第一批）

本轮把先前草拟为死代码的 `BridgeError` 正式接线（NEXT_ACTIONS 结构化错误码项的第一批）。

当前事实：

- `src-tauri/src/lib.rs` 的 `BridgeError`（`code/message/details`，camelCase serde）已接线，共 7 个错误码：
  `invalidArgument / storageError / parseError / networkError / httpStatus / notFound / forbidden`。
- v0.6 OPDS 网络/存储面 7 个命令已从 `Result<_, String>` 迁到 `Result<_, BridgeError>`：
  `opds_add_source / opds_remove_source / opds_list_sources / opds_browse_feed /
  opds_search_feed / opds_ingest_entries / opds_download_epub`。
- 协议已同步：`src/platform/protocol.ts`（`BridgeError` / `BridgeErrorCode` / `isBridgeError`）、
  文档 8 新增「结构化错误码」一节并把冻结清单第 3 项标记「进行中」。`tauri.ts` 无需改动。
- 其余命令（library.*/annotation.*/reading.*/book.*）仍返回字符串，本轮未迁移（非破坏性，可后续逐步迁）。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过。

下一步优先级：

1. 按价值把其余命令逐步迁移到 `BridgeError`（统一 reject 形态），迁完后定稿协议冻结的错误码范围。
2. 给前端按 `code` 分流的实际消费点（如网络错误可重试提示）补一处使用，验证 `isBridgeError` 链路。
3. 继续协议冻结审计其余三项（DTO 预留字段、批量预取语义、资源通道边界）。

## 📌 交接留言（2026-06-18，v0.6 OPDS 当前状态）

先同步 GitHub：

```powershell
cd E:\workspace\game-cooperative-plan\lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

当前事实：

- `main` 已包含 v0.6 OPDS parser / source management / feed browser / OPDS EPUB download acquire pipeline。
- 本轮 `codex/opds-url-detect` 新增书架搜索框 OPDS feed URL 粘贴识别：检测到看起来像 `opds` / `feed` / `catalog` / `.atom` / `.xml` / `.json` 的 `http(s)` URL 时，提示填入 OPDS 源面板；不会自动添加或联网。
- 本轮新增 `npm.cmd run smoke:opds` 真实 Tauri 联网冒烟：默认用 Project Gutenberg `https://www.gutenberg.org/ebooks/search.opds/?query=austen`，覆盖添加源、浏览、进入单本详情、下载 EPUB、入库、打开阅读。
- 本轮只改前端 UI、冒烟脚本与文档，没有改桥接协议、Rust command 或 core schema。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过；仅有 Windows 换行提示。
- `npm.cmd run tauri -- build --debug --no-bundle` 通过。
- `npm.cmd run smoke:opds` 通过；下载并打开 Project Gutenberg《Pride and Prejudice》。

下一步优先级：

1. 将 PR #23 合并后，在 `main` 上按需复跑 `npm.cmd run smoke:opds`。
2. 若继续功能开发，优先推进结构化错误码；同步 `src/platform/protocol.ts`、Rust serde/command 与 `docs/resource-library-plan/8_桥接协议_v0.1.md`。
3. 做协议冻结审计：DTO 预留字段、批量预取语义、结构化错误码、资源通道边界。
4. 保持版权边界：Bangumi / なろう 只做元数据 + 外链；`library.acquireRemote` 仍只允许青空公共版权条目，OPDS 仅允许开放授权/可获取 EPUB 的条目进入下载入库。

## 📌 交接留言（2026-06-18，v0.6 OPDS 第一轮完成）

先同步 GitHub：

```powershell
cd E:\workspace\game-cooperative-plan\lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

当前事实：

- PR #22 已合并进 `main`（`source-record-panel` 分支已删除）。
- v0.6 OPDS 第一轮已在本机完成（未推送，仍在本地 `main`）。
- 本轮改动：Rust core + Tauri commands + protocol + 前端 UI。

本轮已完成（v0.6 OPDS 第一轮）：

- **Rust core**（`crates/reading-core/src/connectors.rs`）：新增 `connectors::opds` 模块。
  - `OpdsLink`/`OpdsEntry`/`OpdsFeed` 结构（serde Serialize/Deserialize，camelCase）。
  - `parse_opds_1x(xml: &str) -> Result<OpdsFeed, String>`：基于 `quick_xml` 的快速 XML 事件解析器，支持 Atom feed、空元素展开、导航子分类识别（`subsection` rel）、封面/获取链接提取（优先缩略图与 EPUB mime type）、权利状态映射（`open_license` / `metadata_only`）。
  - `OpdsSource` 结构（id/name/base_url/enabled）。
  - `list_sources(conn)` / `remove_source(conn, id)` / `search_url(base_url, query)` / `urlencoding(s)` 辅助函数。
  - 5 个单测：采集 feed 解析、导航 feed 解析、权利状态映射、空/垃圾输入边界、端到端落库（`opds_entry_lands_on_shelf_as_remote`）。
- **Tauri shell**（`src-tauri/src/lib.rs`）：6 个 OPDS 命令。
  - `opds_add_source(name, url)`：生成 `opds:md5:` source ID，调用 `ensure_source`。
  - `opds_remove_source(id)` / `opds_list_sources()`。
  - `opds_browse_feed(url)`：HTTP GET + Accept header → parse_opds_1x。
  - `opds_search_feed(source_id, query)`：查 base_url → 构造搜索 URL → fetch + parse。已修复 MutexGuard 跨 await 的 Send trait 问题（db 锁在 await 前 scope drop）。
  - `opds_ingest_entries(source_id, feed)`：过滤导航条目 → ensure_source + ingest → 返回 LibraryBook[]。
  - 全部注册在 invoke_handler。
- **协议**（`src/platform/protocol.ts`）：新增 `OpdsLink`/`OpdsEntry`/`OpdsFeed`/`OpdsSource` DTO，6 个 bridge 方法。
- **平台桥接**（`src/platform/tauri.ts` + `index.ts`）：invoke 包装 + noBridge stub。
- **前端 UI**（`index.html` / `src/styles.css` / `src/main.ts`）：
  - 书架视图新增 `<details>` OPDS 面板（源列表 + 添加表单 + feed 浏览器）。
  - OPDS CSS 样式：源行（名称/URL/操作）、feed 卡片网格（导航卡片 vs 出版物卡片）、标签徽章、空状态。
  - TypeScript 函数：源管理（增删刷新）、feed 浏览（URL 获取 + relative URL 解析）、feed 搜索、条目摄入（单条 + 全部）、外链打开、导航层级穿透。

已验证：

- `cargo test --workspace`：79 passed，0 failed，0 warnings。
- `npm run build`：tsc + vite build 通过。
- `node scripts/check-arch.mjs`：通过（@tauri-apps 仅出现在 src/platform/）。
- `node scripts/check-dev-memory.mjs`：通过。

下一步优先级（v0.6 OPDS 第二轮）：

1. **OPDS 2.0 JSON Feed 支持**：`parse_opds_2x(json)`，OPDS 2.0 使用 JSON-LD/Z39.87 格式，需独立解析器或复用 serde 结构。
2. **实机联网冒烟**：用真实 OPDS 目录站点（如 Standard Ebooks、Feedbooks）测试完整的“添加源 → 浏览 → 导航 → 摄入”流程。
3. **OPDS EPUB 下载 acquire 管线**：`library.acquireRemote` 当前只支持青空文库 XHTML→EPUB 合成；OPDS open_license EPUB 可直接 HTTP 下载后转为本地 asset。
4. **URL 粘贴识别**：书架搜索框粘贴 OPDS feed URL 时自动识别并提示添加为 OPDS 源。
5. **结构化错误码**：协议层的 `ErrorCode` 枚举替代原始字符串错误。
6. **协议冻结审计**：检查桥接协议 8 的 DTO 预留字段是否完整。

或者也可以继续推进 v0.5 遗留项（URL 粘贴识别、结构化错误码、协议冻结），然后切回 v0.6。

## 📌 交接留言（2026-06-17，给寝室电脑 Codex）

先同步 GitHub：

```powershell
cd E:\workspace\game-cooperative-plan\lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

当前事实：

- `main` 已合并到 PR #17 / v0.5-h `catalog_fts`，在线元数据条目已能进入统一目录搜索。
- `main` 已合并 PR #18（远程 metadata 条目 ↔ 本地可读资产人工关联）与 PR #19（`smoke:remote-link` 真窗口冒烟脚本）。
- 发版前最后的原生文件 / 文件夹选择框人工项已补验通过。
- `npm.cmd run package:beta` 已通过，生成 `dist-beta/lightnovel-reader-v0.1.0-release-windows-x64.zip`。
- 如果看到未跟踪的本机文件（例如 `.codex/config.toml`），不要提交，除非明确确认它属于项目配置。

已完成：

- 真实 Tauri 窗口里点击“导入 EPUB”，确认弹出 Windows 原生 `#32770` 文件对话框（标题“打开”）；选择 smoke Vol.1 后，`library_list` 出现 `Smoke Test Light Novel Vol.1`，本地 `user_owned`。
- 真实 Tauri 窗口里点击“导入文件夹”，确认弹出 Windows 原生 `#32770` 文件夹对话框（标题“选择要上传的文件夹”）；选择 smoke fixtures 文件夹后，`library_list` 出现 `Smoke Test Light Novel Vol.2`，本地 `user_owned`。
- `npm.cmd run package:beta` 已通过，生成 `dist-beta/lightnovel-reader-v0.1.0-release-windows-x64` 与 `.zip`。
- `library.linkRemoteToLocal` 已实现第一版：core 移动 `source_record` 到本地 `edition`，保留本地 `asset.id`，隐藏无 asset/无 source_record 的远程空壳；前端远程卡片提供“关联本地”动作，由用户显式确认。
- `npm.cmd run smoke:remote-link` 已新增并通过：真实 Tauri 窗口里导入本地 smoke EPUB、在线搜 AniList `Tanya`、把 `Youjo Senki` 远程卡片关联到本地书，验证远程空壳消失、进度/标注键不变、重复在线搜索不反弹。
- PR #18/#19 合并后，`npm.cmd run package:beta` 已在当前机器重新通过；zip 解压后用 release `reader.exe` 跑 `tauri-webdriver-smoke` 启动冒烟通过。
- `library.listSourceRecords` 已实现第一版：core 可按本地 `asset.id` 或远程 `edition.id` 只读列出挂到同一 edition 的来源记录；书架卡片新增“来源”按钮，展示 AniList/Bangumi/なろう/青空等来源名称、类型、授权/可用状态、remote id 和外链，不下载正文、不自动合并。
- 关联本地书候选已新增可解释排序/提示：合并标题搜索与全量本地候选，按标题、作者、系列、语言、卷号打分，展示匹配理由和低置信/冲突提醒；仍由用户显式确认，不自动合并。
- 批量人工确认队列已新增第一版：当前远程搜索结果可一键整理为队列，每条展示推荐本地候选、匹配理由和冲突提醒；用户逐条“关联/跳过”，不自动合并。

下一步优先级：

1. 若要发便携测试版，可以使用当前 `dist-beta/lightnovel-reader-v0.1.0-release-windows-x64.zip` 作为候选；正式发出前再做一次人工下载/解压/启动抽检即可。
2. 若继续功能开发，下一项建议给真窗口 `smoke:remote-link` 补“来源面板 + 候选排序 + 批量确认队列”断言，防 UI 回归。
3. 后续可继续丰富来源详情只读面板，或进入下一个资源书库体验项；继续遵守版权边界：Bangumi / なろう 只做元数据 + 外链，不做正文抓取；`library.acquireRemote` 仍只允许青空公共版权条目。

## 📌 交接留言（2026-06-17，v0.5-h catalog_fts）

当前分支：`feature/v0.5h-catalog-fts`。PR #16（Bangumi）已合并进 `main`，本分支从最新 `main` 开出。

已完成：

- `crates/reading-core/src/library.rs` 新增 schema 迁移 v5：重建 `catalog_fts(edition_id UNINDEXED, title, author, series_title)`，回填 `edition → volume → series` 实体目录，并用 `edition/volume/series` 触发器同步更新。
- `library::search_books` 的 ≥3 字路径从旧 `books_fts` 改走 `catalog_fts`，本地 asset 与远程 metadata-only 条目都能被标题/作者/系列命中；短词 LIKE 兜底保持不变。
- 测试覆盖：schema 版本升到 5；v4 旧库远程条目升级后回填进 `catalog_fts`；远程 metadata-only 条目可被标题/作者/系列长词搜索命中，且标题/作者更新会同步 FTS。
- 协议 8、DEV_LOG、DECISIONS 已同步。

已验证：

- `cargo test -p reading-core` 通过（73 passed）。
- `cargo test --workspace` 通过（reading-core 73 passed）。
- `npm.cmd run build` 通过。
- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `git diff --check` 通过（仅 Windows 换行提示）。

当前状态：

- 已合并进 `main`（PR #17）。本段保留为历史交接记录；继续开发请看本文顶部“给寝室电脑 Codex”。

## 📌 交接留言（2026-06-17，v0.5-g Bangumi 元数据源）

本轮 Codex 继续 v0.5-g，新增 **Bangumi（中文/ACG 元数据）** 在线找书来源。当前分支：`feature/v0.5g-bangumi-metadata`。

已完成：

- core：`crates/reading-core/src/connectors.rs` 新增 `connectors::bangumi`，构造 Bangumi OpenAPI `POST /v0/search/subjects` 请求体（`type=[1]` 书籍、`nsfw=false`），解析 `id/name/name_cn/short_summary/summary/images`，落库为 `rights_status=unknown`、`availability=remote`、`remoteUrl=https://bgm.tv/subject/<id>`；补 4 个单测。
- shell：`src-tauri/src/lib.rs` 的 `library_search_remote_source` 新增 `source=bangumi`，HTTP POST 留在 Tauri 壳侧，解析/落库仍在 core。
- frontend/protocol/docs：`RemoteLibrarySource` 扩展为 `anilist|bangumi|aozora|narou`，在线找书下拉新增“Bangumi（中文/ACG 元数据）”；`unknown` 远程卡片文案改为“远程条目 · 外链”，避免把 Bangumi 误称为官方授权入口；协议 8、DEV_LOG、DECISIONS 已同步。

已验证：

- `cargo test --workspace` 通过（reading-core 72 passed）。
- `npm.cmd run build` 通过。
- Bangumi OpenAPI 轻量探测通过：经系统代理 POST `https://api.bgm.tv/v0/search/subjects?limit=2` 返回 HTTP 200，含 `type=1` 书籍 subject。
- `npm run tauri dev` 真窗口 + WebView2 CDP 冒烟通过：在线来源下拉确认四源 AniList / Bangumi / 小説家になろう / 青空文库；Bangumi 搜索 `狼与香辛料` 返回远程卡片，卡片为 `book-card-remote`，有远程封面，标签为“远程条目 · 外链”。

边界：

- Bangumi 只做社区/目录型元数据 + subject 外链，不做正文抓取、不缓存正文、不标记为官方授权入口。
- `library.acquireRemote` 仍只支持青空 `public_domain` 条目；不要让 Bangumi/なろう 进入正文获取路径。
- HTTP 继续留在 src-tauri 壳侧；解析/落库在 `reading-core`；前端继续只通过 `src/platform`。

当前状态：

- 已合并进 `main`，且后续 `catalog_fts` 与发版前原生选择框人工项也已完成。本段保留为历史交接记录；继续开发请看本文顶部“给寝室电脑 Codex”。

## 📌 交接留言（2026-06-17，给实验室电脑 Codex）

先同步 GitHub：`git fetch --all --prune`，切到最新 main 后查看/接续 `feature/v0.5f-narou-metadata` / PR #14。本轮 Codex 已完成 **v0.5-f 小説家になろう官方 API 元数据源**，并在后续接手中补过真窗口联网冒烟：

- core：`crates/reading-core/src/connectors.rs` 新增 `connectors::narou`，解析官方 API JSON（`allcount` 汇总行跳过，`ncode/title/writer/story` 映射为 `RemoteEntry`），落库为 `rights_status=official_free`、`availability=remote`，补 3 个单测。
- shell：`src-tauri/src/lib.rs` 的 `library_search_remote_source` 新增 `source=narou`，壳侧用 reqwest GET 官方 API；HTTP 留在壳，解析/落库仍在 core。
- frontend/protocol/docs：`RemoteLibrarySource` 扩展为 `anilist|aozora|narou`，在线找书下拉新增“小説家になろう（Web小说元数据）”；协议 8、DEV_LOG、DECISIONS 已同步。
- 验证已跑：`cargo test --workspace`（reading-core 68 passed）、`npm.cmd run build`、`node scripts/check-arch.mjs`、`node scripts/check-dev-memory.mjs`、`git diff --check`；另用官方 API 做过一次 HTTP 200 轻量探测。
- 2026-06-17 接手补验：`npm run tauri dev` 真窗口 + WebView2 CDP 联网 smoke 通过 AniList / なろう / 青空三源；なろう `転生` 返回“官方免费 · 外链”远程条目（样例 `https://ncode.syosetu.com/n6316bn/`），青空 `羅生門` acquire 后可打开，青空非公共版权与无 HTML URL 样例均拒绝下载。为适配本机 Windows 用户代理，`src-tauri/Cargo.toml` 的 reqwest 增加 `system-proxy` feature。

当前状态：

- 已合并进 `main`，且 Bangumi、`catalog_fts`、发版前原生选择框人工项也已完成。本段保留为历史交接记录；继续开发请看本文顶部“给寝室电脑 Codex”。

## 📌 交接留言（2026-06-16，给寝室电脑的 Claude / 下一会话）

你好。这是另一台机器上的 Claude 留的言。**第一件事：`git pull`**——main 已经领先你本地不少。

本轮（已全部合并进 main，PR #6/#7/#8/#9/#11/#12/#13）完成了 v0.5 资源/元数据层从地基到在线来源扩展的主链：

- **v0.5-a/b**（PR #6）：实体模型 `series/volume/edition/asset` + `source/source_record` 落地（迁移 v3），从 books 回填 + 导入双写。`asset.id = 内容哈希`，标注/进度键不动。
- **v0.5-c**（PR #7）：读路径**锚定 edition**（一个版本 = 一个书架条目），不再读 books；迁移 v4 把 `thumb_path` 迁到 asset；`LibraryBook.filePath/fileSize` 转可选。books 退为只读镜像（v0.6 可 DROP）。
- **v0.5-d**（PR #9）：**首个元数据连接器 AniList** + 「在线找书」UI。core `connectors.rs`（查询/解析/落库，纯函数可测、无网络）+ 壳 `reqwest` 命令 `library_search_remote`（HTTP）。远程条目只展示封面/简介、点击跳官方（版权红线）。
- **v0.5-e**（PR #11/#12/#13）：青空文库作为**公共版权经典文学**来源接入在线找书，并完成 `library.acquireRemote` 公共版权正文获取管线；青空不再表述为轻小说主来源。
- **v0.5-f**（当前分支）：小説家になろう官方 API 接入在线找书，定位为 Web 小说元数据 + 官方入口，`official_free` 远程条目，不做正文获取。
- **工具**（PR #8）：项目级 `.mcp.json` 接入 context7（实时文档）。**装好后想用要说 "use context7"**。

**当前状态**：v0.5-f 实现中；本轮收工前需跑 `cargo test --workspace`、`npm.cmd run build`、check-arch / check-dev-memory / diff-check。

**建议你接着做（任选，按价值排序）**：

1. **实机验证**（推荐先做）：`npm run tauri dev` 真窗口里验证「🌐 在线找书」的 AniList/なろう/青空文库来源切换。AniList 是轻小说/ACG 商业元数据入口；なろう是官方 Web 小说元数据入口，条目应显示“官方免费 · 外链”并跳官方 ncode 页面；青空文库是公共版权经典文学入口，首次搜索应按需下载目录并缓存，公共版权条目应显示“公共版权经典 · 可站内读”，点击后获取官方 XHTML/HTML、合成为 cached asset 并能直接打开阅读。需联网。
2. **青空 acquire 实机补验**：找一条 `rightsStatus=public_domain` 且带 ruby/插图的青空经典文学条目，确认正文 ruby 保留、图片/HTML 安全清洗不破坏阅读；再找非公共版权/无 HTML URL 条目确认命令层拒绝下载并只走外链。
3. `catalog_fts`：让远程条目可全文搜（现在 books_fts 不含它们，远程条目只能 LIKE 短词命中）。
4. 真正贴近轻小说的后续连接器：优先评估 Bangumi（中文/ACG 元数据）。カクヨム/Royal Road/正文抓取类来源必须先过 ToS 审核，倾向 v0.7 插件运行时而非内核连接器；なろう metadata 已由 v0.5-f 接入。
4. 仍挂着的人工项：原生文件/文件夹选择对话框点一次，然后 `npm run package:beta` 发版。

细节看 `DEV_LOG.md` 与 `DECISIONS.md`（2026-06-16 青空连接器、acquire、なろう metadata 决策）。协议变更在 `docs/resource-library-plan/8_桥接协议_v0.1.md`（新增/保留 `library.searchRemote`、`library.searchRemoteSource`、`library.acquireRemote`、`shell.openExternal`、`remoteUrl`、`rightsStatus`）。

---

## 进度快照（2026-06-13）

**已完成（4 个主题提交，分支已推送，[PR #1](https://github.com/haryqs/lightnovel-reader/pull/1) 待合并）：**

- v0.2 阅读内核 + v0.3 本地书库（导入·去重·封面·元数据·FTS 搜索·最近阅读）。
- v0.3.1 core 加固：SQLite 迁移框架、持久化解析缓存、章节 HTML 安全清洗（防 XSS）、
  EPUB 解析健壮性。**reading-core 测试 47 全过**。
- 前端 UI 减法（简明大方）、NSIS 安装器配置、二次元插画资源。
- **三套自动冒烟全绿**：`smoke:tauri`（UI 启动）/ `smoke:p0`（桥接+重启恢复+解析缓存+图片）/
  `smoke:p1`（开书·翻页·划词高亮+重开渲染·真实 Calibre 读取）。
- **真实 NSIS 安装器/卸载器静默装卸验证通过**（≈7.4MB）。
- 文档整理 37 → 23（建 `docs/README.md` 索引、合并去重）。

**v0.3.1 发版前人工项状态（2026-06-17 更新）：**

- PR #1 以来的主线内容已进入 `main`。
- 原生文件 / 文件夹选择对话框已在真实 Tauri 窗口补验通过。
- `npm.cmd run package:beta` 已生成便携测试包。

## P0.5：打包发版（配置就绪，已验证一次）

- ✅ `npm.cmd run tauri build` 出 NSIS `LightNovel Reader_0.3.1_x64-setup.exe` + MSI（≈7.4MB）。
- ✅ 静默安装 `/S` → `%LOCALAPPDATA%\LightNovel Reader\` + 开始菜单快捷方式 + `uninstall.exe`；
  静默卸载 `/S` 安装目录与快捷方式干净移除。
- 待发版前确认一次：卸载是否保留 `%APPDATA%` 用户书库（本轮用隔离临时数据目录，未在真实数据目录下验证）。
- 待决（非阻塞，1.0 前）：占位 `identifier`（`com.tauri-app.reader`）是否改正式域名
  （会迁移 `$APPDATA` 书库路径，趁无真实用户时改）；是否上 Tauri updater 做「检查更新」。
- 不做：账号登录器（撞离线优先/不做 SaaS）、脱离连接器的下载器（撞合规红线）。
- 命令与清单统一见 `docs/current-project/发布与测试.md`。

## P1：v0.3 本地书库补齐

- 实机确认书架封面、系列、语言展示符合预期。
- 实机确认批量导入失败详情可读、不会阻塞后续导入。
- 实机确认 Calibre 已降级为“更多导入来源 / 从 Calibre 迁移”，不再是空书架主路径。
- 实机确认轻小说书架视觉在真实窗口内无重叠、按钮不挤压、空状态可操作。
- 实机确认动态插图不过度干扰阅读，进入正文后背景层正确淡出。
- ~~路线审阅。~~ 已完成（2026-06-13），结论见
  `docs/current-project/12_Claude路线审阅意见_2026-06-13.md`，
  路线获认可，三条新决策已入 `DECISIONS.md`。
- 更新 v0.3 状态文档，准备把本地书库闭环标为基本可用。

## P2：v0.4 标注增强与性能打磨

- ~~改进跨元素选区高亮。~~ 已完成（2026-06-13，Claude）：computeAnchor 改用
  `cloneContents().textContent` 计偏移（与定位用的 textContent 同口径，修块级边界 `\n` 错位）；
  创建走 `applyAnnotationHighlight` 按偏移逐文本节点包裹，跨段落选择即时高亮；smoke:p1 确定性验证渲染为多段 mark。
- ~~增加 JSON 导出。~~ 已完成（2026-06-13，Claude）：`exportAnnotationsJson`（完整结构化、
  含 anchor/时间戳）+ 标注侧栏 MD/JSON 双导出按钮；smoke:p1 拦截 blob 端到端校验通过。
- ~~增强 text hash fallback 定位。~~ 已完成（2026-06-13，Claude）：`locateAnnotationOffset`
  收集所有 exact 出现位置，用 prefix/suffix 上下文消歧 + 保存时 start 就近兜底，
  修「正文重复文本永远高亮第一个」错位。
- ~~封面缩略图（导入时生成小尺寸封面）+ 书架懒加载。~~ 已完成（2026-06-16，Claude）：
  `image` 依赖（png/jpeg，fail-open）导入时生成 ≤240×360 缩略图；迁移框架 v2 加 `thumb_path` 列；
  DTO/协议加 `thumbPath`；书架优先缩略图 + `loading="lazy"`。cargo 49 全过 + smoke:p0 真窗口断言缩略图。
- 批量导入并行流水线（rayon）+ 进度 UI；桌面文件夹导入改走路径版 `library.import`。

## P2.5：安全加固（v0.4 内）

- ~~章节 HTML 安全清洗（防 XSS：script/事件属性/js: URL/iframe 等）。~~ 已完成
  （2026-06-13，Claude）：`html_sanitizer::sanitize_security`，10 个安全单测。详见 DECISIONS.md。
- 评估收紧 `src-tauri/tauri.conf.json` 的 `csp`（现 null）——纵深防御；需实机确认不破坏
  `reader-img://` 自定义协议与内联样式，未实机不改。
- 评估引入 `ammonia`（html5ever 白名单清洗）替代字符串扫描级清洗（需记入 DECISIONS）。
- ~~EPUB 解析健壮性：多 rootfile/缺 spine/非 zip 等边界。~~ 已完成（2026-06-13，Claude）：
  修复多 rootfile 取最后一个的 bug，补 6 个边界测试。
- ~~持久化解析缓存：解析/清洗结果落盘，二次开书读缓存不重解析。~~ 已完成
  （2026-06-13，Claude）：`crates/reading-core/src/parse_cache.rs`，按 bookId 落盘
  BookInfo + 清洗后章节 HTML，已接入 Tauri 开书/取章；fail-open，29 个 cargo test 全过。
  仍待：实机量二次开书提速幅度；删书时清理对应缓存目录。
  （分页结果落盘未做——分页依赖前端视口尺寸，宜留前端层缓存，不在 core。）

## P3：v0.5 实体模型与在线元数据

- `books` 单表迁移为 `series / volume / edition / asset / source / source_record`
  实体模型（见 DECISIONS.md 2026-06-13；annotations / reading_state 键不动）。
  → 迁移框架已就位（`crates/reading-core/src/migrations.rs`），实体模型作为
  library `MIGRATIONS` 的 version 2 追加即可。
  → schema 草案已就绪：`docs/resource-library-plan/10_书库实体模型_v0.5_schema草案.md`
  （建表 + 回填 SQL + DTO 演进 + 分四步迁移顺序，可直接采纳）。
- ~~reading-core 补按 `PRAGMA user_version` 顺序执行的最简迁移框架。~~ 已完成
  （2026-06-13，Claude）：`migrations::run` 按 user_version 升序、每条独立事务原子
  推进；library/storage 现有 schema 收为基线 v1；24 个 cargo test 全过。
- `LibraryBook` DTO 终稿（预留 `seriesId/volumeId/editionId` 可选字段）。
- remote_only / metadata_only 条目的书架 UI 规范（与本地书的视觉区分）。
- 设计官方链接识别器。
- 设计来源授权状态字段（`asset.availability` × `source_record.rights_status` 正交拆分）。
- 设计轻小说元数据源：作品、系列、卷、版本、语种、画师、出版社、官方入口。
- 设计 OPDS / 私有书库连接器入口，优先服务用户自有或合法授权来源。
- 协议冻结前完成文档 8「冻结前检查清单」四项（DTO 预留 / 批量预取语义 /
  结构化错误码 / 资源通道核对）。

## P4：工具与协作

- 重启 Codex 后确认全局 curated skills 已出现在可用技能列表中。
- 评估是否创建每日/每周文档审计 automation。
- 将关键工作拆给子代理前，先明确写入范围和冲突边界。
