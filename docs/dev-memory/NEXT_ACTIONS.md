# 下一步任务队列

## P0：环境与验证

- 当前机器已完成 `git pull origin main`、`npm.cmd install`、`cargo test --workspace`、`npm.cmd run build` 基础验证。
- `npm.cmd run tauri dev` 已确认可启动 Vite 与 `reader.exe`，但当前 Codex shell 未暴露可见 Tauri 主窗口，仍需在可交互桌面窗口做冒烟测试。
- ✅ `npm.cmd run smoke:tauri`（UI 启动自动冒烟）**已跑通**（2026-06-13，Claude）：
  标题/品牌/插画加载/默认主题 light/书库浮层（导入 EPUB/文件夹/搜索齐全、Calibre 在折叠面板内）/关闭。
- ✅ `npm.cmd run smoke:p0`（P0 桥接深度自动冒烟）**已跑通**（2026-06-13，Claude）：
  导入(vol1/vol2)/去重/元数据/封面/library_list/library_open/get_chapter/解析缓存落盘/
  **内联图片解码 256×320**/进度保存/标注保存/**第二会话重启后进度+标注+缓存全部恢复**；
  首开 11.33ms vs 二开 7.39ms（解析缓存提速）。
  过程中定位并修复一处坏测试样本 PNG（GDI+ 宽容、Chromium 严格拒解码），reader-img 传输零损耗确认无 bug，
  详见 DEV_LOG 2026-06-13 同日条目。
- ⚠️ 自动化**不覆盖**：系统文件/文件夹选择器、真实翻页热区、鼠标划词创建高亮的 UI 交互、真实
  Calibre 库迁移——这几项仍需按 doc 13 人工补一遍，补完即可打 v0.3.1。
- 冒烟样本 / 便携包 / Web 安装器 / 打包 / 安装卸载验证：统一见 `docs/current-project/发布与测试.md`。
- 验证：打开 EPUB、翻页、关闭重开恢复进度、高亮重开仍可见。
- 验证：“本地书库”中的“导入 EPUB”和“导入文件夹”可导入单本/多本 EPUB。

## P0.5：v0.3.1 测试版打包（安装器 + 卸载器）

> Claude 已预配好 `src-tauri/tauri.conf.json`（productName=LightNovel Reader、
> version=0.3.1、NSIS 当前用户安装/中英双语、publisher/描述/分类齐全）。
> 以下步骤需在**可见桌面 + Rust 工具链**环境由 Codex 执行验证（Claude 无法实机验证）。

- 先完成 P0 实机冒烟，**冒烟通过才打包**，别发未验证的包。
- 运行 `npm.cmd run tauri build`，确认在 `src-tauri/target/release/bundle/` 下产出：
  - NSIS 安装器 `nsis/LightNovel Reader_0.3.1_x64-setup.exe`（自带卸载器）。
  - MSI 安装器 `msi/*.msi`（如不需要可在 bundle.targets 改为 `["nsis"]` 只出 NSIS）。
- 实机验证安装器全流程：安装 → 桌面/开始菜单图标 → 启动 → 控制面板可正常卸载。
- 确认卸载后 `%APPDATA%` 下的书库数据按预期保留（卸载不应删用户书库）。
- 待决（非阻塞，发版前确认一次）：
  - `identifier` 仍是占位 `com.tauri-app.reader`，决定 1.0 前是否改为正式域名
    （改 identifier 会变更 `$APPDATA` 路径，迁移现有 `library.sqlite`，趁无真实用户时改）。
  - 是否接入 Tauri updater 插件做「检查更新」（替代独立登录器/启动器，见 12 号审阅意见）。
- 不做：账号登录器（与离线优先/不做 SaaS 冲突）、脱离连接器的下载器（撞合规红线）。

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
- 增加 JSON 导出。
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
