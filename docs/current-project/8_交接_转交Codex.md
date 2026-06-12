# 交接:Claude Code → GPT Codex(reader 开发)

> 日期 2026-06-11。截至提交 `4938121`,工作树干净,所有测试通过。
> 本文是完整交接:架构与不可变决策、已完成、未验证项、下一步、已知陷阱。
> 启动时请先读 `AGENTS.md`(自动加载)和本文件;改 `reader/` 代码前必读第三节的纪律。

## 一、项目定位(一句话)

开源轻小说/电子书阅读器:本地优先 + Calibre 兼容;终局多端(桌面三系统已覆盖,
Android/iOS 硬性目标,鸿蒙远期)+ 开放源插件生态。**合规路线:索引聚合 + 合法来源,
内核零内置抓取源**(推导见 `workspace/reader-resource-library-plan/1_产品定位与边界.md`、`4_版权与合规边界.md`)。

## 二、架构(已收敛,不要重开辩论)

**一份 Rust 核心(reading-core)+ 一份 TS 阅读引擎 + N 个薄平台壳(系统 WebView)。**

- 排版必须用 CSS 引擎(轻小说的 ruby/竖排/禁则,自写排版不可行)→ 各端系统 WebView。
- Tauri v2 = **第一代桌面壳**,经桥接协议隔离、可整体替换;移动端 v0.8/v0.9 计划
  Tauri mobile 优先、手写 Kotlin/Swift 壳兜底。
- Flutter/Servo/纯 Rust GUI 均已否决,否决理由记录在
  `workspace/reader-resource-library-plan/7_终局架构_多端与插件运行时.md` 第三节——翻案前先读它。
- 插件运行时(v0.7)双引擎:桌面/Android 用 QuickJS,**iOS 必须用系统 JavaScriptCore**
  (App Store 审核指南 2.5.2:下载的代码只许 WebKit/JSC 执行)。插件契约已冻结在
  `reader/plugin-sdk/`,按契约写运行时即可。

## 三、强制纪律(违反 = 返工)

1. **前端禁止直接 import `@tauri-apps/*`**,一律经 `reader/src/platform/` 适配层。
   `reader/scripts/check-arch.mjs` 已接入 `npm run build`,违规直接构建失败。
2. **业务逻辑写进 `reader/crates/reading-core`**,Tauri command 只做参数搬运。
3. **改协议 = 同时改三处**:`reader/src/platform/protocol.ts`(TS 类型)+
   reading-core 的 serde 结构 + `workspace/reader-resource-library-plan/8_桥接协议_v0.1.md`(文档)。
   wire 字段一律 camelCase,**禁止单字段 serde rename**(历史教训见第六节陷阱 1)。
   协议 v0.5 冻结 1.0,冻结前可自由改。

## 四、当前状态(截至 4938121)

### 已完成并提交

| 提交 | 内容 |
|---|---|
| `4697277` | 方案文档 0–9 入库(含终局架构 7、桥接协议 8、插件契约 9) |
| `04f052c` | cargo workspace 化:reading-core 独立 crate(解析/清洗/存储下沉) |
| `bec607c` | 前端 platform 适配层 + check-arch 强制检查 |
| `d68fdba` | 插件契约 SDK(manifest schema + host-api.d.ts + 青空文库示例骨架) |
| `21d47bd` | 阅读进度持久化(reading_state 表 + reading.* 协议 + 引擎防抖保存/开书恢复) |
| `a2ffcac` | 书库 core 模块 v1(books 表 + SHA-256 对象仓库去重 + FTS5 trigram 搜索) |

功能面:EPUB 解析/HTML 清洗/虚拟页分页/双页/三主题/高亮批注+Markdown 导出/
进度持久化/Calibre 书库浏览全部可用(桌面)。

### 验证状态(诚实声明)

- ✅ `cargo test --workspace` 15/15 通过(标注/进度/书库导入去重/FTS 搜索/排序)
- ✅ `npm run build`(含 check-arch + tsc)通过;`cargo check --workspace` 干净
- ⚠️ **未做开窗冒烟测试**(本轮全部在无头环境)。接手后第一件事:`npm run tauri dev`
  实测:开书 → 翻页 → 关闭重开是否回到原位置;高亮保存后重开是否还在
  (kind/type 修复后标注持久化是**第一次**真正可用,务必验证)。

## 五、下一步(按优先级)

1. **冒烟测试**(上面 ⚠️ 项)。
2. **v0.3 书库接线闭环**:
   - 协议加 `library.import` / `library.list` / `library.search` / `library.touchLastRead`
     (+ 按 id 开书的入口,可复用 `book.openPath` 传对象仓库路径);
   - src-tauri 加对应 command,接 `reading-core::library`(library.sqlite 放
     app_data_dir,AppState 加一个独立的 `Mutex<Connection>`);
   - 书架 UI:自有书库视图(导入按钮/列表/搜索框),Calibre 降级为"导入来源"
     (list_calibre_books → 批量 import_epub);
   - 打开书库书籍后调 `touch_last_read`。
3. **书库 v1.1 补缺**:封面提取(OPF manifest cover-image → covers/ 目录,
   books.cover_path 回填);series 元数据(OPF 里的 `calibre:series` meta)。
4. **v0.4 标注增强**(跨元素选区高亮目前降级跳过,见 annotations.ts applyHighlight)。
5. v0.5 在线元数据 + **协议冻结 1.0**;v0.7 插件运行时(rquickjs,按 plugin-sdk 契约)。

## 六、已知陷阱(都踩过一次了,别再踩)

1. **serde 单字段 rename 引发的两侧漂移**:标注的 `kind` 曾被 Rust 重命名为 `type`,
   前端发 `kind` → 反序列化必败 → 高亮静默存不进库(调用处没 catch)。已修复
   (wire 统一 `kind`,数据库列名仍是 `type`)。这就是纪律 3 的由来。
2. **`reader-img` 协议在 Windows 的特殊形态**:WebView2 下必须是
   `http://reader-img.localhost/<path>`,不是 `reader-img://localhost/...`。
   已在 `html_sanitizer.rs` 用 `#[cfg(windows)]` 处理;新壳实现该 scheme 时注意。
3. **FTS5 trigram 最少 3 字**:`library::search_books` 对 <3 字查询走 LIKE 兜底,
   这是设计而非疏漏;不要"统一成 FTS"。默认 unicode61 对 CJK 不可用,勿换回。
4. **bookId 双端同源**:前端 `computeBookId`(annotations.ts)与 core
   `compute_book_id`(lib.rs)都是 SHA-256 前 32 hex,改任何一边必须同步另一边。
   书库 books.id 也是同一哈希 → 进度/标注自动跟书走。
5. **Gutenberg 式 EPUB 单章 6 万 px**:虚拟页分页就是为它做的,改分页逻辑前先用
   大单章书回归(历史问题记录在 `7_交接_ClaudeCode进度与下一步.md`)。
6. **Tauri command 参数命名**:invoke 传 camelCase(如 `bookId`),Tauri 自动映射到
   Rust 的 snake_case 参数,不要手动改名。
7. **cargo 命令在 `reader/` 工作区根跑**(Cargo.lock 在 reader/,不在 src-tauri/)。

## 七、文档地图

- `AGENTS.md` / `CLAUDE.md` —— AI 入口(两份内容已同步,改一份请同步另一份)
- `workspace/reader-resource-library-plan/` 文档 0–9 —— 产品定位/书库/合规/路线图/
  终局架构/桥接协议/插件契约(**7、8、9 是本次新增的权威架构文档**)
- `当前项目_阅读器/1_技术决策_选型推导.md` + `AI协作指南.md` —— 早期已收敛决策
- `reader/plugin-sdk/README.md` —— 插件作者视角的契约说明
