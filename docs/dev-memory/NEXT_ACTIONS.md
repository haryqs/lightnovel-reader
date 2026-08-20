# 下一步任务队列

## 📌 交接留言（2026-08-20，v0.7.0 正式五资产已验收）

当前事实：

- npm、Tauri 与三个第一方 Cargo 包版本已统一为 `0.7.0`，运行时 User-Agent 从 Cargo 包版本生成；
  协议 `1.0-rc.1` 与 `gutenberg@0.1.0` 保持独立版本。
- 新增 `check:version` / `test:version`，并将版本一致性接入项目检查、生产构建、beta/Web 安装器与正式分发入口。
- 新增 Windows GitHub Actions 质量门，覆盖 npm 项目门、发布回归、前端构建、Cargo workspace 与 QuickJS 测试。
- 旧 `v0.3.1` 草稿 Release 与 RC5 资产不再是公开候选；签名安装器和 `latest.json` 不能只改名复用。
- 重复嵌套目录已移到可恢复备份 `C:\Users\41267\Documents\Codex\2026-08-18\n\work\lightnovel-reader-duplicate-backup-20260818`；
  三个内容未变化的 sync 假修改已从工作区状态清除。
- PR #46 已合并到 `main`（`504635b`），仓库已切换为 Public；许可证、版本门与 CI 均已进入发布目标。
- Billing 概览现为 GitHub Free、$0 用量、无下次付款和 0/2000 Actions 分钟；PR #47 的 push 与
  pull_request 两条 Windows CI 已真正执行并全绿，证明 Actions runner 后台锁已解除。Support #4676102
  可保留到 GitHub 回复后关闭。
- 新增统一五资产验收器：不读取私钥，自动核对应用版本/tag/URL、精确资产白名单、updater `.sig` 引用、
  插件 SHA-256/大小与编译内公钥 Ed25519 签名；旧 v0.3.1 URL、篡改包和额外密钥文件均有回归阻断。
- PR #47 已合并。首次公开前发现旧 updater 私钥密码不可用；因仓库尚无公开 Release/Tag，已保留旧密钥并
  轮换新 updater 密钥，`tauri.conf.json` 已切换到新公钥。新密码只由维护者保存，不进入仓库或日志。
- `E:\lightnovel-reader-release-staging\v0.7.0-release` 已生成并通过统一验收，只含五个公开资产；正式 NSIS
  静默安装、启动、卸载均通过，卸载前后两份书库文件大小和 SHA-256 不变。
- 新 updater 公钥已通过 PR #48 合并到 `main`（`cb75a53`）。新的 `v0.7.0` Draft Release 已以该合并提交
  为目标上传五资产；远端资产集合、大小与 GitHub SHA-256 digest 全部匹配本机候选，草稿仍未公开。

下一步优先级：

1. **人工审阅草稿**：复核 v0.7.0 发布说明、AGPL 许可证、目标提交 `cb75a53` 和五资产；草稿 URL 的
   `untagged-*` 是未公开状态，不能用于在线更新结论。
2. **明确同意后公开**：只有维护者明确批准才发布 v0.7.0；不得把“草稿上传完成”当作公开授权。
3. **发布后在线复验**：公开后验证 `/releases/download/v0.7.0/` 的 `latest.json`、插件仓库下载和真实更新重启；
   完成前不宣称在线更新闭环通过。

## 📌 交接留言（2026-08-18，补齐开源许可证发布门）

当前事实：

- 根目录已补入 SPDX 官方 `AGPL-3.0-only` 完整正文；`package.json`、`package-lock.json` 和三个 Cargo 包
  均声明 `AGPL-3.0-only`，Cargo 包同时指向项目仓库。
- 新增 `check:license` 与 `test:license`：标准许可证正文或 npm/Cargo 元数据漂移会失败；项目检查、生产构建、
  beta/Web 安装器和正式 Tauri 分发入口均已接入该门。
- 本轮 `check:project`、生产构建、许可证/发布信任/updater 回归、Cargo workspace 与 QuickJS 特性测试均通过；
  workspace 为 Tauri 8 passed / 1 个公网测试 ignored、reading-core 149 passed。
- 当前分支仍比 `origin/main` 多 updater 点号资产名修复，许可证收口也尚未推送；草稿 Release 目标仍为 `main`。
- 工作区的三个 sync 文件内容哈希与 Git 索引一致，仅显示换行/索引状态；`.codex/.codex`、`docs/docs`、
  `public/public`、`scripts/scripts`、`tools/tools` 是未跟踪的重复目录，本轮未删除。

下一步优先级：

1. **同步发布目标**：审阅并提交许可证收口，把 updater 点号资产名修复与本轮提交合入远端 `main`；
   确认草稿发布目标包含两者后才允许公开。
2. **仓库公开准备**：确认重复目录确实可删后清理；将 GitHub 仓库从 Private 改为 Public，复核 GitHub 能识别
   `AGPL-3.0` 许可证，并把草稿安装器文案改为实际点号资产名。
3. **公开与在线更新**：人工公开五资产草稿，从旧版本完成检查、下载、安装、重启与用户数据保留复验。

## 📌 交接留言（2026-08-04，v0.3.1 RC5 草稿 Release 已就绪）

当前事实：

- FlClash 已开启 DNS 覆写，并通过 `+.gutenberg.org` fake-IP 排除让系统解析恢复真实公网 IP
  `152.19.134.47`；SSRF 防护保持不变。
- 正常公网下旧 `/ebooks/search/` HTML 入口已不返回搜索结果。插件现使用 Gutenberg 官方
  `/ebooks/search.opds/` Atom feed，按 `.opds` 书目 id 提取作品，并按 `rel=next` 判断分页。
- 宿主 User-Agent 更新为 `LightNovelReader/0.3.1 source-plugin-host`，附带项目仓库联系地址；
  继续沿用同域最少 1 秒请求间隔。
- 新增 Gutenberg OPDS 无网络回归，覆盖搜索、详情、章节和 EPUB 提案；真实公网忽略测试也已完成
  `search → getBook → getChapter → acquire`，成功定位 `11.epub3.images`。
- 公开前已将 `gutenberg-test` 改为正式 `gutenberg` 来源，显示名为
  `Project Gutenberg`，首个公开版本为 `0.1.0`，跟踪资产为 `gutenberg.zip`。
- 正式候选由 `lnr-plugin-2026-01` 签名并通过独立公钥复验，SHA-256 为
  `76f715e85e6360c9a8e0f7ec5bfe5fdaaed26b74221388d4da0d4fc074b0f692`。
- GitHub 会把 NSIS 资产名中的空格改为点号；`prepare:updater-release` 已固化该规则并增加回归，
  避免 `latest.json` 指向不存在的带空格资产。
- 插件候选位于 `E:\lightnovel-reader-release-staging\v0.1.0-gutenberg`；最终五文件统一候选位于
  `E:\lightnovel-reader-release-staging\v0.3.1-release-rc5`。插件签名、updater URL/签名及五文件哈希均已复验。
- GitHub PR #45 已合并到 `main`；`v0.3.1` 草稿 Release 已创建，目标为 `main`，
  五个 RC5 资产状态均为 uploaded，GitHub 返回 SHA-256 与本机逐项一致；草稿尚未公开。

下一步优先级：

1. **最终确认与公开**：人工确认草稿标题、说明和五个 RC5 资产后点击发布；
   不要上传 RC/RC2/RC3 中的旧 `gutenberg-test` 或带空格 updater 资产。
2. **真实在线更新**：NSIS 安装/启动/卸载及数据保留已通过；Release 尚未公开，因此旧版本的检查、
   下载、安装和重启仍待发布后执行，不要提前宣称在线更新通过。
3. **MSI 环境后续**：在不阻断 updater 的前提下检查/修复本机 Windows Installer 服务，再单独运行
   `tauri build --bundles msi --no-sign`；WiX 中文代码页配置已经修复，不得退回 1252。

## 📌 交接留言（2026-07-25，首批正式信任根已激活）

当前事实：

- 维护者已在仓库外生成两套独立私钥：插件仓库 Ed25519 私钥与带密码的 Tauri updater 私钥；私钥和密码均未进入仓库。
- 插件公钥 `lnr-plugin-2026-01` 已写入 `src-tauri/src/plugin_trust.rs`，并开启
  `REQUIRE_OFFICIAL_PLUGIN_SIGNATURES=true`；官方索引从此不再接受 unsigned 条目。
- Tauri updater 公钥已写入 `src-tauri/tauri.conf.json`，并开启 `bundle.createUpdaterArtifacts=true`。
- `check:release-trust` 已扩展为四项门禁；`test:release-trust`、实际仓库门禁与 `cargo check -p reader` 已通过。

下一步优先级：

1. **签署首批官方仓库**：确定准备发布的插件 zip 与 HTTPS URL，生成最终 SHA-256/大小索引；由维护者在本地调用
   `sign:plugin-repository` 注入私钥路径，输出所有条目都带 `keyId=lnr-plugin-2026-01` 的签名索引。
2. **首次 updater 发布演练**：在受控构建环境通过 `TAURI_SIGNING_PRIVATE_KEY`（值可为私钥路径）与
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 注入秘密，运行 `release:build`，检查 Windows 安装器与 `.sig`，
   生成 GitHub Release `latest.json`，并从旧版本真实执行检查、下载、安装和重启。
3. **公网与安装复验**：在正常公网 DNS 下完成签名仓库加载/预览/二次下载安装、Gutenberg 获取与阅读；
   抽检 NSIS 安装/卸载保留用户数据。窗口自动化仍沿用既有限制，不补丁 WDIO `node_modules`。

## 📌 交接留言（2026-07-21，发布信任门与 Windows WDIO 调研）

当前事实：

- v0.7 插件来源收口已在 `codex/v0.7-release-hardening` 建立检查点提交 `8a8f83f`。
- 新增 `check:release-trust`：正式分发前同时检查官方插件强制验签、非空且合法的 Ed25519 公钥 keyring，
  以及非空的 Tauri updater 公钥；插件包签名和应用更新签名是两个独立信任域，不能共用或互相替代。
- `package:beta`、`installer:web` 的 npm pre-hook 及新的 `release:build` 都接入该门禁。当前仓库会按设计阻断，
  直到维护者注入正式公钥并开启 `REQUIRE_OFFICIAL_PLUGIN_SIGNATURES`；开发构建与测试不受影响。
- `test:release-trust` 已覆盖合法配置、三项未配置、重复 keyId、非法 Base64 和错误公钥长度。
- 评估过官方文档建议的 `@wdio/tauri-service` embedded provider，但未提交实验集成：`1.2.0` 固定的
  `@wdio/native-utils@2.4.0` 缺少其导入的 `installMockSyncOverride`；升级到 2.5.0 后又发现 Windows EdgeDriver
  版本解析只接受 `MSEdgeDriver`，而当前官方二进制输出 `Microsoft Edge WebDriver`，导致完全匹配的 150.0.4078.83
  仍被判定为 unknown、重复下载，最终嵌入会话也不稳定。不要在项目内补丁 `node_modules`。

下一步优先级：

1. **配置正式信任材料**：由维护者在离线环境生成插件 Ed25519 发布密钥和 Tauri updater 签名密钥，私钥进入独立秘密管理；
   只提交两个信任域各自的公钥。签署官方仓库全部 zip、开启强制插件验签后，让 `npm.cmd run check:release-trust` 变绿。
2. **受控发布演练**：在正常公网 DNS 下运行真实 HTTPS 仓库加载/预览/安装二次下载、Gutenberg 获取与阅读；随后运行
   `npm.cmd run release:build`，抽检 updater 签名、便携包、NSIS 安装/卸载保留用户数据。
3. **窗口自动化后续**：优先等待/升级到修复上述两个 Windows 问题的 WDIO Tauri service，或在上游提交最小复现；
   在此之前继续保留现有 `tauri-driver` smoke 和离线签名回归，不维护项目内依赖补丁。

## 📌 交接留言（2026-07-20，v0.7 正式插件来源流程与 WASM 构建修复）

当前事实：

- 已执行 `git fetch --all --prune`；本地 `main` 与 `origin/main` 同为 `a03103c`，GitHub 没有新提交。
- Tauri 现已显式启用 `reading-core/quickjs`，之前未被真实编译的 runtime 路径已修正。
- QuickJS 运行时已对齐 SDK：`export default`、Promise、标量入参、`HttpResponse.text()`、
  `host.html`、持久化 `host.kv`、可读 JS 异常堆栈、DTO 校验和章节 HTML 清洗。
- `plugin.testFlow` 和插件面板“测试”按钮现会自动跑完 `search → getBook → getChapter`，而不是只跑 `search`。
- 插件 HTTP 沙箱禁止自动重定向，解析后拒绝本机/内网/保留地址，并固定已校验 DNS 结果；
  HTTP 响应体/HTML 输入/返回 JSON 上限 8 MiB，单条日志上限 4 KiB。
- 无网络 runtime 全流程测试通过；Project Gutenberg 示例和 ignored 联网 E2E 已补。当前 Codex 环境把
  `www.gutenberg.org` 解析到 `198.18.0.15` 保留网段，因内网防护被预期拒绝；没有为了跑测试放宽安全策略。
- 浏览器端 `reading-core` 的 wasm-bindgen 产物已重新生成并纳入仓库；`build:wasm` 会校验锁定版本后重复生成，
  `check:wasm` 已接入 `check:project` 和生产构建，干净检出不再因缺模块而失败。
- 已新增 `source.list/search/getBook/getChapter/collect`，不改变 `plugin.testFlow` 诊断语义。启用插件会进入在线来源选择器，
  支持分页搜索、详情/章节、纯文本正文预览和显式收藏；搜索不自动落库，收藏会重新执行 `getBook` 后由 core 幂等写来源记录。
- 插件返回 URL 现在也必须属于 manifest 精确域名；单页搜索结果、章节数和文本长度有硬上限。
  离线 `scripts/test-plugin` manifest 已对齐 SDK 并增加确定性正式来源调用测试。
- QuickJS 可选 `acquire(remoteId, mode)` 与 additive `source.acquire` 已落地：仅放行
  `public-domain/open-license + application/epub+zip`，宿主复核授权/域名，经共享限速/SSRF 下载器获取并验证 EPUB，
  然后挂入远程 edition 的本地 `cached` asset；Gutenberg 示例和 UI “获取并阅读”已同步。
- 官方仓库 Ed25519 包字节验签已落地：索引先校验可信 keyId，预览/安装在各自下载与 SHA-256 后重新验签；
  `sign:plugin-repository` 从外部 PKCS#8 私钥签署 zip。当前编译内正式 keyring 仍为空，unsigned 条目显示人工白名单 warning。
- `smoke:plugin-repository-signature` 已把临时密钥、真实 zip、正式签名工具、篡改/错 key 拒绝及 core/Tauri 验签测试串成
  无公网/GUI 的可重复回归；真实 HTTPS 下载与正式 keyring 仍未复验。
- 仓库 WebDriver smoke 已恢复 npm 入口并收紧为本地包失败即失败；当前 WebView2 150 环境即使用精确匹配驱动也会在
  会话创建后断开 DevTools，且既有来源 smoke 同样复现，窗口自动化复验暂受环境阻断。

下一步优先级：

1. **真实窗口公网复验**：离线正式流程已由 `npm.cmd run smoke:plugin-source` 在真实 `reader.exe` 自动通过；
   仍需在正常公网 DNS 的 Windows/Tauri 环境安装
   `plugin-sdk/examples/gutenberg-test/gutenberg-test.zip`，完成搜索、章节预览与“获取并阅读”；或运行
   `cargo test -p reader plugin_executor::tests::runs_gutenberg_search_book_chapter_acquire_flow -- --ignored --nocapture`。
   当前 Codex 环境再次实跑仍因 fake-IP/保留地址被 SSRF 门预期拒绝，不得为测试放宽。
2. **发布密钥门槛**：离线生成正式 Ed25519 发布密钥，私钥进入独立秘密管理；只把公钥 Base64 和 keyId 加入
   `src-tauri/src/plugin_trust.rs`，签署官方索引全部 zip，验证轮换/撤销流程后把 `REQUIRE_OFFICIAL_PLUGIN_SIGNATURES` 切为 `true`；
   随后用受控 HTTPS 仓库跑索引加载、预览、安装二次下载及篡改/未知 key 的真实窗口测试。
3. **分发复验**：继续便携包目标机器抽检与 NSIS 卸载保留用户数据验证。

## 📌 交接留言（2026-06-27，11 commits，全 Phase 完工）

> **实验室交接：** 以下是从上次会话到现在的完整进展。新会话进入后请先读 AGENTS.md，然后按开工纪律操作。

### 本轮完成（11 个 commit，全部推送到 GitHub）

**Phase 1-4 + 扩展：**

| Commit | 内容 |
|--------|------|
| `2f1069d` | Phase 1: WASM 网页端 MVP（reading-core native/wasm 拆分 + Rust 分页 + web-bridge） |
| `8f99d20` | Phase 2: 自托管同步服务器（crates/sync-server, axum + SQLite, 设备配对码） |
| `c453122` | Phase 3: 桌面端独立化（系统托盘 + 关闭到托盘 + .epub 文件关联 + 自动更新） |
| `9f9e8c1` | Phase 4: GPU 翻页动画 + PWA（CSS transform 双缓冲 + vite-plugin-pwa） |
| `25aea3e` | QuickJS 插件运行时（rquickjs, 一次性 Runtime, host.http/kv/log 沙箱） |
| `38d9d74` | QuickJS HTTP executor + 25s 中断超时 |
| `1487489` | host.kv 持久化到 plugin_store（kv.json per 插件） |
| `dee0a3a` | 冷启动计时 + measure-cold-start.mjs 测量脚本 |
| `f22cf8b` | Tauri sync 命令对接 sync-server（6 个命令：pair/join/status/unpair/push/pull） |
| `d0bc56b` | 前端 tauri.ts 接入真实 sync 命令 |
| `846d763` | QuickJS 插件测试 UI（已安装插件列表加"测试"按钮 + plugin_test_run 命令） |

**验证状态：** `cargo test` 137 passed, `npm run build` 18 modules + PWA, tsc 零错误, check-arch/check-protocol-freeze OK.

### 立即可做（按优先级）

1. **端到端同步测试：** 启动 sync-server (`cargo run -p sync-server -- sync.db 0.0.0.0:9876`)，开桌面端 + 网页端配对测试
2. **插件完整流程：** 写一个实际可用的 test 插件（搜索/获取书/获取章），用"测试"按钮跑通全流程
3. **自动同步轮询：** 桌面端/网页端定时 syncNow()，目前是手动触发
4. **RELEASE BUILD 验证：** `npm run tauri build` 产生产品构建，实测冷启动 < 1s

### 开工操作

```bash
cd /c/Users/Administrator/lightnovel-reader
git fetch --all --prune
git pull --ff-only
git status -sb
npm install
cargo check --workspace
npm run build
```

## 📌 交接留言（2026-06-27，Hermes 三层架构三线并行收工）

先同步 GitHub：

```bash
cd /c/Users/Administrator/lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

本轮 Hermes（DeepSeek v4 Pro）担任 Tech Lead，调度 Claude Code + OpenCode 三线并行：

**任务 A（Claude Code -p）：QuickJS 集成架构方案**
- 产出：`docs/resource-library-plan/9_插件运行时_QuickJS集成方案.md`（187 行）
- 核心决策：选 `rquickjs`（绑定 quickjs-ng），每次调用一个一次性 Runtime，JSON 序列化对接现有 DTO，HTTP 经 `PluginHttpExecutor` trait 穿壳层（保持 core 无网络纪律），双层超时（QuickJS 中断 + tokio::timeout 30s），沙箱只注入四个 host 命名空间
- 状态：纯方案文档，未实现

**任务 B（OpenCode + Sonnet via OpenRouter）：插件仓库 smoke 测试**
- 产出：`scripts/tauri-plugin-repository-smoke.mjs`（592 行）+ `docs/testing/plugin-repository-smoke-limitations.md`（109 行）
- package.json 新增 `smoke:plugin-repo` script
- 覆盖：插件包检查、安装流程、启用/禁用、卸载、错误处理
- 限制：HTTPS 强制导致无法测真实网络仓库；不执行插件 JS 代码

**任务 C（OpenCode + DeepSeek）：文档更新** — 失败（幻觉）
- DeepSeek 模型在 DEV_LOG 中虚构了"完成端到端测试"和"plugin_runtime 模块"——已回滚
- 教训：文档/记录类任务不交给弱模型，由 Hermes 直接操作

**环境**：Windows 10, Claude Code CLI 2.1.179（Pro OAuth）, OpenCode 1.17.11（DeepSeek API + OpenRouter）

已验证：
- `node scripts/check-arch.mjs` 通过
- `node scripts/check-dev-memory.mjs` 通过
- `node scripts/check-protocol-freeze.mjs` 通过
- `node --check scripts/tauri-plugin-repository-smoke.mjs` 通过

下一步优先级：
1. Review QuickJS 集成方案文档 → 确认 rquickjs 选型 → 进入实现（可再委托 Claude Code）
2. 构建 Tauri debug 版并跑 smoke:plugin-repo（需要 GUI 环境）
3. 补上用户自装 vs 官方白名单插件视觉区分
4. 继续用三层架构：Hermes 写 Spec → Claude Code 做复杂实现 → OpenCode 做测试/文档

## 📌 交接留言（2026-06-23，v0.7 官方插件仓库下载校验链路进行中）

先同步 GitHub：

```powershell
cd E:\workspace\game-cooperative-plan\lightnovel-reader
git fetch --all --prune
git checkout main
git pull --ff-only
git status -sb
```

当前事实：

- 当前工作分支：`codex/plugin-repository-install-flow`。
- 本轮在官方插件仓库索引骨架上补了 UI/下载/校验/安装确认链路：书库“源插件（v0.7 预览）”面板可输入官方索引 JSON URL、加载候选插件、逐条校验包并复用现有安装预览/确认区域。
- 新增桥接能力：`plugin.repository.load`、`plugin.repository.inspectPackage`、`plugin.repository.installPackage`；前端仍只通过 `src/platform/ReaderBridge`，Tauri command 只做 HTTPS 下载、大小检查和参数搬运，core 负责索引/包校验、预览、落盘。
- 官方索引仍额外门槛：拒绝 `user-declared`；拒绝重复插件 id；包地址/源码地址必须 HTTPS；包 SHA-256 必须 64 hex；包大小最大 50 MiB；`official-free + acquire` 在 ToS/限速/用户确认门控落地前不得进入官方索引或安装。
- 仓库包预览和安装都会校验 `packageSha256`；安装命令会重新下载并再次校验，不信任预览缓存。
- 签名字段当前只校验 `ed25519/keyId/value` 形状并返回 warning，不做密码学验签；不要对外宣称“已签名验证”。
- 当前仍不执行插件 JS，不引入 QuickJS；官方仓库下载的是插件包元数据/入口文件安装流，不是正文抓取流。
- 已新增 `npm.cmd run smoke:plugin-repository-fixtures`：生成合法插件 zip、SHA-256 与 `repository.json`，用于后续真实窗口 smoke；它不会启动 HTTPS 服务。
- 官方仓库候选“源码”按钮已补错误回显，平台外链打开失败会显示到插件面板。
- 加载新的官方索引会先清空旧安装预览，避免索引上下文切换后误安装上一轮已校验包。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `node scripts/check-protocol-freeze.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 123 passed）。
- `npm.cmd run smoke:plugin-repository-fixtures -- --out-dir .\tmp-plugin-repository-smoke --base-url https://plugins.example.invalid/smoke` 通过，测试产物已删除。
- `git diff --check` 通过（仅 Windows 换行提示）。

下一步优先级：

1. 优先给官方索引安装流补真实窗口 smoke：复用 `smoke:plugin-repository-fixtures` 产物，接入测试 HTTPS server 或可信 HTTPS fixture URL，验证加载索引、校验包、安装确认、已安装列表刷新。
2. 继续保持用户自装插件与官方白名单插件视觉区分；UI 后续可补官方来源 badge、索引 warning 展示细节和源码入口文案。
3. 真正做签名验签前，需要单独实现 keyring/验签逻辑并更新 DECISIONS；当前 signature 只是元数据预留。
4. 如要放行某个 `official_free` 源正文获取，先补源站 ToS 记录、限速策略、用户确认与单源测试；不要把正文抓取放进内核连接器。

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

## 📌 交接留言（2026-06-27，Phase 2 同步服务 v1 完工）

本轮完成 Phase 2：
- `reading-core::sync` 模块（277行/6测试）：冲突解决算法（LWW+墓碑复活）
- migration v7（library DB）+ v2（storage DB）：sync 列 + sync_outbox 表 + 触发器
- `crates/sync-server`（457行）：axum REST + WebSocket，设备配对码认证
- 桥接协议新增 syncPair/syncStatus/syncNow/syncUnpair，web-bridge 实现，sync-pairing UI
- `cargo test` 137 passed；`npm run build` 18 modules OK

**下一步（Phase 3）：桌面端独立化**
- 系统托盘 + 关闭到托盘（tauri-plugin-tray）
- .epub 文件关联 + single-instance
- 自动更新（tauri-updater）
- 冷启动 < 1s（预构建 + 延迟加载）
- Tauri 端实现 sync 命令对接 sync-server
- 桌面端内置零配置局域网同步模式

**下一步（Phase 4）：性能打磨**
- GPU 翻页动画（CSS transform 双缓冲）
- PWA 接入（vite-plugin-pwa）
- 真实文本测量评估（目前仍用字符数启发式）
