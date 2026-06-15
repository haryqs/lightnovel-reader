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
