# 技术路线与 MVP

## 总体技术路线

延续当前阅读器项目已经收敛的路线：

- Tauri v2
- Rust 负责 EPUB 解析、HTML 清洗、文件扫描、数据库访问
- TypeScript 负责 UI、分页、交互
- SQLite 作为本地书库数据库
- 本地优先，不依赖服务器
- AGPL v3

## 分阶段目标

### v0.2：阅读内核

目标：先把本地阅读体验打牢。

功能：

- Rust EPUB 解析。
- HTML 清洗。
- 前端 CSS Columns 分页。
- 阅读进度持久化。
- 目录导航。
- 三套护眼主题。

不做：

- 在线源。
- 插件市场。
- 大规模书库管理。

### v0.3：本地书库

目标：替代 Calibre 作为内部书库模型。

功能：

- `library.sqlite`
- 本地文件导入。
- SHA256 去重。
- 书架列表。
- 系列、卷、版本基本字段。
- 封面缓存。
- 阅读状态与最近阅读。

技术重点：

- SQLite schema 设计。
- 文件对象仓库。
- EPUB 元数据抽取。

### v0.4：标注与导出

目标：让阅读器成为知识工具。

功能：

- 高亮。
- 批注。
- 书签。
- Markdown 导出。
- JSON 导出。
- 标注定位 fallback。

技术重点：

- CFI / DOM locator。
- text hash fallback。
- 标注 schema。

### v0.5：在线元数据与链接

目标：开始接入在线，但只做元数据和合法入口。

功能：

- 粘贴链接识别。
- 官方页面记录。
- 购买/借阅/免费阅读入口。
- 元数据补全。
- 授权状态标记。

不做：

- 正文抓取。
- 盗版源。
- 绕过平台限制。

### v0.6：OPDS

目标：接入标准电子书目录。

功能：

- OPDS 1.x 订阅。
- OPDS 2.0 试验支持。
- 目录浏览。
- 搜索。
- 开放授权内容下载。
- 授权状态识别。

### v0.7：Source Adapter

目标：建立可控的在线来源扩展机制。

功能：

- source adapter manifest。
- 权限声明。
- 来源风险标记。
- 官方白名单。
- 本地私有 adapter。

不做：

- 默认开放无审核插件市场。

## MVP 建议

最小可行版本不要碰在线资源。

MVP 应该是：

```text
本地 EPUB 导入
本地书库 SQLite
系列/卷管理
阅读进度
护眼主题
目录
标注
Markdown 导出
```

理由：

- 先证明阅读体验和书库模型可用。
- 在线接入会放大版权、维护、适配难度。
- 没有强阅读内核，资源再多也留不住用户。

## 首批 schema 草案

```sql
CREATE TABLE series (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  original_title TEXT,
  description TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE work (
  id TEXT PRIMARY KEY,
  series_id TEXT,
  title TEXT NOT NULL,
  kind TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE volume (
  id TEXT PRIMARY KEY,
  series_id TEXT,
  work_id TEXT,
  volume_number REAL,
  title TEXT NOT NULL,
  subtitle TEXT,
  original_language TEXT,
  release_date TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE edition (
  id TEXT PRIMARY KEY,
  volume_id TEXT NOT NULL,
  language TEXT,
  publisher TEXT,
  translator TEXT,
  isbn TEXT,
  edition_name TEXT,
  rights_status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE asset (
  id TEXT PRIMARY KEY,
  edition_id TEXT,
  kind TEXT NOT NULL,
  sha256 TEXT,
  file_path TEXT,
  source_url TEXT,
  mime_type TEXT,
  size_bytes INTEGER,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE source (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  base_url TEXT,
  risk_level TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE reading_state (
  id TEXT PRIMARY KEY,
  asset_id TEXT NOT NULL,
  locator TEXT NOT NULL,
  progress_percent REAL NOT NULL DEFAULT 0,
  last_read_at TEXT NOT NULL
);

CREATE TABLE annotation (
  id TEXT PRIMARY KEY,
  asset_id TEXT NOT NULL,
  locator TEXT NOT NULL,
  selected_text TEXT,
  text_hash TEXT,
  note TEXT,
  color TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

## 技术风险

- SQLite schema 过早复杂化。
- 轻小说元数据来源不稳定。
- 不同语种、译名、别名合并困难。
- EPUB 标注定位在字号变化、HTML 清洗后漂移。
- 在线接入带来版权和维护成本。
- 移动端文件系统权限复杂。

## 推荐策略

先做强阅读器，再做书库，再做在线。

顺序不能反：

```text
阅读体验 -> 本地书库 -> 标注导出 -> 元数据补全 -> 合法在线入口 -> 插件生态
```

