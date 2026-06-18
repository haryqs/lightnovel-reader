# 下一步任务队列

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
- 本轮只改前端 UI 与文档，没有改桥接协议、Rust command 或 core schema。

已验证：

- `node scripts/check-arch.mjs` 通过。
- `node scripts/check-dev-memory.mjs` 通过。
- `npm.cmd run build` 通过。
- `cargo test --workspace` 通过（reading-core 84 passed）。
- `git diff --check` 通过；仅有 Windows 换行提示。

下一步优先级：

1. 真实联网 OPDS 冒烟：用真实 OPDS 目录站点验证“添加源 → 浏览 → 导航 → 下载 EPUB → 入库 → 打开阅读”。
2. 若冒烟暴露错误信息不清晰，推进结构化错误码；同步 `src/platform/protocol.ts`、Rust serde/command 与 `docs/resource-library-plan/8_桥接协议_v0.1.md`。
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
