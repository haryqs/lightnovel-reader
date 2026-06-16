# 下一步任务队列

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

**距 v0.3.1 发版仅剩：**

1. 合并 [PR #1](https://github.com/haryqs/lightnovel-reader/pull/1) 到 main。
2. 人工点一次原生文件/文件夹选择对话框（约 20 秒；导入逻辑已由 smoke:p0 路径版证过，自动化够不着原生对话框）。
3. `npm.cmd run package:beta` 出便携测试包发版。

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

- 改进跨元素选区高亮。
- ~~增加 JSON 导出。~~ 已完成（2026-06-13，Claude）：`exportAnnotationsJson`（完整结构化、
  含 anchor/时间戳）+ 标注侧栏 MD/JSON 双导出按钮；smoke:p1 拦截 blob 端到端校验通过。
- 增强 text hash fallback 定位。
- 封面缩略图（导入时生成小尺寸封面）+ 书架懒加载。
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
