# 决策日志

> 记录影响未来开发方向的取舍。格式：日期 / 决策 / 理由 / 后果。

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

- `LibraryBook` 新增可选 `rightsStatus`；前端用它区分「公共版权 · 可站内读」与「需购买/官方外链」。
- 新增协议 `library.searchRemoteSource(source, query)` 与 `library.acquireRemote(id)`；旧 `library.searchRemote(query)` 保留为 AniList 兼容入口。
- 青空目录按需下载，缓存 7 天；只在用户主动选择青空文库搜索时触发，不影响只用 AniList/本地书库的用户。
- 若未来要支持多章 HTML、青空 txt 标记或非 EPUB 可读资产，应另开设计，不在本决策里隐式扩展。
