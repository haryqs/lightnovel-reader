//! 自有书库（v0.3 起替代 Calibre 作为内部书库模型）。
//!
//! Schema v1 刻意保持单表：轻小说最要紧的 series/卷号先以平面字段表达，
//! 完整的 系列/作品/卷/版本 关系模型（方案文档 5 草案）等有真实需求时
//! 经 `PRAGMA user_version` 迁移升级，避免过早复杂化。
//!
//! 搜索：FTS5 trigram（CJK 子串可命中，方案文档 7 的结论——默认 unicode61
//! 会把连续汉字并成单 token，对中日文基本不可用）。查询不足 3 字时 trigram
//! 无法分词，回退 LIKE。

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;

use crate::migrations::{self, Migration};
use crate::{compute_book_id, epub_parser};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBook {
    /// = SHA-256(epub 字节) 前 32 hex，与阅读器 bookId 同源 → 进度/标注直接对上。
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub language: Option<String>,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub description: Option<String>,
    /// 库内对象路径（objects/<id>.epub）。远程 metadata_only 条目无文件 → None。
    pub file_path: Option<String>,
    /// 文件字节数。远程条目 → None。
    pub file_size: Option<i64>,
    pub cover_path: Option<String>,
    /// 小尺寸缩略图（covers/<id>_thumb.png），书架优先加载它而非原图。无则为 None。
    pub thumb_path: Option<String>,
    pub added_at: i64,
    pub last_read_at: Option<i64>,
    // ── v0.5 实体模型可选字段（JOIN asset/edition/volume 回填；本地库恒有值）。──
    // wire 层只「新增可选字段」，旧前端忽略即可，符合协议冻结规则。
    /// 系列 id（'series:'名 / 'solo:'bookId）。供书架系列聚合视图。
    pub series_id: Option<String>,
    /// 卷 id（'vol:'bookId）。
    pub volume_id: Option<String>,
    /// 版本 id（'ed:'bookId）。
    pub edition_id: Option<String>,
    /// 资产可得性（local|remote|missing|cached）。远程元数据条目据此决定能否站内读。
    pub availability: Option<String>,
    /// 授权状态（user_owned/public_domain/official_purchase/unknown...）。
    pub rights_status: Option<String>,
    /// 来源外链（受版权/远程条目点击后跳官方页）。本地条目为 None。
    pub remote_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySourceRecord {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub source_kind: String,
    pub remote_id: Option<String>,
    pub remote_url: Option<String>,
    pub rights_status: String,
    pub availability: Option<String>,
    pub last_checked_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteAcquisition {
    pub edition_id: String,
    pub source_id: String,
    pub remote_id: String,
    pub rights_status: String,
    pub title: String,
    pub author: Option<String>,
    pub language: Option<String>,
    pub existing_asset_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    pub book: LibraryBook,
    /// true = 内容哈希已存在，未重复入库（返回已有记录）。
    pub duplicate: bool,
}

/// 基线 schema（迁移 v1）。版本号由迁移框架经 `PRAGMA user_version` 盖戳，
/// 此处不再硬编码 pragma。`IF NOT EXISTS` 让框架上线前的旧库被幂等补盖。
const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS books (
  id            TEXT PRIMARY KEY,
  title         TEXT NOT NULL,
  author        TEXT,
  language      TEXT,
  series        TEXT,
  series_index  REAL,
  description   TEXT,
  file_path     TEXT NOT NULL,
  file_size     INTEGER NOT NULL,
  cover_path    TEXT,
  added_at      INTEGER NOT NULL,
  last_read_at  INTEGER
);

CREATE VIRTUAL TABLE IF NOT EXISTS books_fts USING fts5(
  title, author, series,
  content='books', content_rowid='rowid',
  tokenize='trigram'
);

CREATE TRIGGER IF NOT EXISTS books_ai AFTER INSERT ON books BEGIN
  INSERT INTO books_fts(rowid, title, author, series)
  VALUES (new.rowid, new.title, new.author, new.series);
END;
CREATE TRIGGER IF NOT EXISTS books_ad AFTER DELETE ON books BEGIN
  INSERT INTO books_fts(books_fts, rowid, title, author, series)
  VALUES ('delete', old.rowid, old.title, old.author, old.series);
END;
CREATE TRIGGER IF NOT EXISTS books_au AFTER UPDATE ON books BEGIN
  INSERT INTO books_fts(books_fts, rowid, title, author, series)
  VALUES ('delete', old.rowid, old.title, old.author, old.series);
  INSERT INTO books_fts(rowid, title, author, series)
  VALUES (new.rowid, new.title, new.author, new.series);
END;
"#;

/// v0.5 实体模型（schema 草案 §4/§5）：books 单表 → 系列/卷/版本/资产 四层图谱
/// + 来源层（source/source_record，供 v0.5 连接器写入）。
///
/// 关键不变量：`asset.id = books.id`（EPUB 内容哈希）→ annotations / reading_state 以
/// 同一键关联，本次迁移完全不动它们。books 表本周期保留为只读回滚保险，v0.6 再 DROP。
///
/// 回填用确定性派生 id（'series:'+名 / 'vol:'+bookId / …）+ `INSERT OR IGNORE`，
/// 迁移可重入、同系列多卷只建一行 series。source/source_record 建空表（本地导入无来源记录），
/// catalog_fts 的同步触发器由 v5 迁移重建补齐。
const ENTITY_SCHEMA_V3: &str = r#"
CREATE TABLE IF NOT EXISTS series (
  id          TEXT PRIMARY KEY,
  title       TEXT NOT NULL,
  title_sort  TEXT,
  author      TEXT,
  description TEXT,
  cover_path  TEXT,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS volume (
  id            TEXT PRIMARY KEY,
  series_id     TEXT REFERENCES series(id) ON DELETE SET NULL,
  kind          TEXT NOT NULL DEFAULT 'main',
  volume_number REAL,
  title         TEXT NOT NULL,
  subtitle      TEXT,
  description   TEXT,
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_volume_series ON volume(series_id, volume_number);

CREATE TABLE IF NOT EXISTS edition (
  id            TEXT PRIMARY KEY,
  volume_id     TEXT REFERENCES volume(id) ON DELETE CASCADE,
  language      TEXT,
  publisher     TEXT,
  translator    TEXT,
  isbn          TEXT,
  edition_name  TEXT,
  rights_status TEXT NOT NULL DEFAULT 'user_owned',
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_edition_volume ON edition(volume_id);

CREATE TABLE IF NOT EXISTS asset (
  id            TEXT PRIMARY KEY,
  edition_id    TEXT REFERENCES edition(id) ON DELETE SET NULL,
  kind          TEXT NOT NULL DEFAULT 'epub',
  availability  TEXT NOT NULL DEFAULT 'local',
  file_path     TEXT,
  file_size     INTEGER,
  cover_path    TEXT,
  added_at      INTEGER NOT NULL,
  last_read_at  INTEGER
);
CREATE INDEX IF NOT EXISTS idx_asset_edition ON asset(edition_id);
CREATE INDEX IF NOT EXISTS idx_asset_lastread ON asset(last_read_at);

CREATE TABLE IF NOT EXISTS source (
  id             TEXT PRIMARY KEY,
  name           TEXT NOT NULL,
  kind           TEXT NOT NULL,
  base_url       TEXT,
  license_policy TEXT,
  risk_level     TEXT,
  enabled        INTEGER NOT NULL DEFAULT 1,
  created_at     INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS source_record (
  id              TEXT PRIMARY KEY,
  source_id       TEXT REFERENCES source(id) ON DELETE CASCADE,
  entity_type     TEXT NOT NULL,
  entity_id       TEXT NOT NULL,
  remote_url      TEXT,
  remote_id       TEXT,
  rights_status   TEXT NOT NULL DEFAULT 'unknown',
  availability    TEXT,
  last_checked_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_source_record_entity ON source_record(entity_type, entity_id);

CREATE VIRTUAL TABLE IF NOT EXISTS catalog_fts USING fts5(
  title, author, series_title,
  tokenize='trigram'
);

-- 回填：每行 books 拆成 series ← volume ← edition ← asset 一条链。
-- series_id 同名归并（'series:'+名），无系列名的书各自独立（'solo:'+bookId）。
INSERT OR IGNORE INTO series(id, title, title_sort, author, description, cover_path, created_at, updated_at)
  SELECT
    CASE WHEN series IS NOT NULL AND series <> '' THEN 'series:'||series ELSE 'solo:'||id END,
    COALESCE(NULLIF(series, ''), title), NULL, author, description, cover_path, added_at, added_at
  FROM books;

INSERT OR IGNORE INTO volume(id, series_id, kind, volume_number, title, subtitle, description, created_at, updated_at)
  SELECT
    'vol:'||id,
    CASE WHEN series IS NOT NULL AND series <> '' THEN 'series:'||series ELSE 'solo:'||id END,
    'main', series_index, title, NULL, description, added_at, added_at
  FROM books;

INSERT OR IGNORE INTO edition(id, volume_id, language, publisher, translator, isbn, edition_name, rights_status, created_at, updated_at)
  SELECT 'ed:'||id, 'vol:'||id, language, NULL, NULL, NULL, NULL, 'user_owned', added_at, added_at
  FROM books;

INSERT OR IGNORE INTO asset(id, edition_id, kind, availability, file_path, file_size, cover_path, added_at, last_read_at)
  SELECT id, 'ed:'||id, 'epub', 'local', file_path, file_size, cover_path, added_at, last_read_at
  FROM books;
"#;

/// 书库数据库的迁移序列。新增列/表一律追加新版本，绝不改 SCHEMA_V1
/// （旧库已盖戳 v1，不会再跑 v1）。
/// v4：缩略图从 books 迁到 asset。读路径切实体锚定后不再读 books，封面/缩略图须从
/// asset 取；故给 asset 补 thumb_path 列并从 books 回填。books 仍保留该列（只读镜像）。
const ASSET_THUMB_V4: &str = "\
    ALTER TABLE asset ADD COLUMN thumb_path TEXT; \
    UPDATE asset SET thumb_path = (SELECT thumb_path FROM books WHERE books.id = asset.id) \
      WHERE thumb_path IS NULL;";

const CATALOG_FTS_V5: &str = r#"
DROP TABLE IF EXISTS catalog_fts;
CREATE VIRTUAL TABLE catalog_fts USING fts5(
  edition_id UNINDEXED,
  title,
  author,
  series_title,
  tokenize='trigram'
);

INSERT INTO catalog_fts(edition_id, title, author, series_title)
  SELECT e.id, v.title, s.author, CASE WHEN s.id LIKE 'series:%' THEN s.title END
    FROM edition e
    JOIN volume v ON v.id = e.volume_id
    JOIN series s ON s.id = v.series_id;

CREATE TRIGGER IF NOT EXISTS catalog_fts_edition_ai AFTER INSERT ON edition BEGIN
  DELETE FROM catalog_fts WHERE edition_id = new.id;
  INSERT INTO catalog_fts(edition_id, title, author, series_title)
    SELECT new.id, v.title, s.author, CASE WHEN s.id LIKE 'series:%' THEN s.title END
      FROM volume v
      JOIN series s ON s.id = v.series_id
     WHERE v.id = new.volume_id;
END;

CREATE TRIGGER IF NOT EXISTS catalog_fts_edition_ad AFTER DELETE ON edition BEGIN
  DELETE FROM catalog_fts WHERE edition_id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS catalog_fts_edition_au AFTER UPDATE OF volume_id ON edition BEGIN
  DELETE FROM catalog_fts WHERE edition_id = old.id;
  INSERT INTO catalog_fts(edition_id, title, author, series_title)
    SELECT new.id, v.title, s.author, CASE WHEN s.id LIKE 'series:%' THEN s.title END
      FROM volume v
      JOIN series s ON s.id = v.series_id
     WHERE v.id = new.volume_id;
END;

CREATE TRIGGER IF NOT EXISTS catalog_fts_volume_ad AFTER DELETE ON volume BEGIN
  DELETE FROM catalog_fts
   WHERE edition_id IN (SELECT id FROM edition WHERE volume_id = old.id);
END;

CREATE TRIGGER IF NOT EXISTS catalog_fts_volume_au AFTER UPDATE OF title, series_id ON volume BEGIN
  DELETE FROM catalog_fts
   WHERE edition_id IN (
     SELECT id FROM edition WHERE volume_id = old.id OR volume_id = new.id
   );
  INSERT INTO catalog_fts(edition_id, title, author, series_title)
    SELECT e.id, new.title, s.author, CASE WHEN s.id LIKE 'series:%' THEN s.title END
      FROM edition e
      JOIN series s ON s.id = new.series_id
     WHERE e.volume_id = new.id;
END;

CREATE TRIGGER IF NOT EXISTS catalog_fts_series_au AFTER UPDATE OF title, author ON series BEGIN
  DELETE FROM catalog_fts
   WHERE edition_id IN (
     SELECT e.id
       FROM edition e
       JOIN volume v ON v.id = e.volume_id
      WHERE v.series_id = new.id
   );
  INSERT INTO catalog_fts(edition_id, title, author, series_title)
    SELECT e.id, v.title, new.author, CASE WHEN new.id LIKE 'series:%' THEN new.title END
      FROM edition e
      JOIN volume v ON v.id = e.volume_id
     WHERE v.series_id = new.id;
END;
"#;

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: SCHEMA_V1,
    },
    // v2：封面缩略图列。已在 v1 的旧库经 ALTER 补列；新库 v1 建表后 v2 补列。
    Migration {
        version: 2,
        sql: "ALTER TABLE books ADD COLUMN thumb_path TEXT;",
    },
    // v3：v0.5 实体模型（系列/卷/版本/资产 + 来源层）+ 从 books 回填。
    Migration {
        version: 3,
        sql: ENTITY_SCHEMA_V3,
    },
    // v4：缩略图迁到 asset（读路径实体锚定的前置）。
    Migration {
        version: 4,
        sql: ASSET_THUMB_V4,
    },
    // v5：实体目录 FTS（本地 + 远程 metadata），让 library.search 覆盖远程条目。
    Migration {
        version: 5,
        sql: CATALOG_FTS_V5,
    },
];

pub fn open_library(db_path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(db_path)?;
    migrations::run(&conn, MIGRATIONS)?;
    Ok(conn)
}

/// 导入一本 EPUB：内容哈希去重 → 复制进对象仓库（objects/<id>.epub）→ 建档。
/// 元数据从 OPF 抽取（v1 只有 title/author；series/封面后续版本补）。
pub fn import_epub(
    conn: &Connection,
    library_dir: &Path,
    epub_path: &Path,
    now_ms: i64,
) -> Result<ImportOutcome, String> {
    let data = std::fs::read(epub_path).map_err(|e| format!("读取文件失败: {}", e))?;
    let file_name = epub_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned());
    import_epub_bytes(conn, library_dir, &data, file_name.as_deref(), now_ms)
}

/// 从文件选择器/移动端沙盒传入的 EPUB 字节导入书库。
/// 这样 UI 不依赖平台是否能暴露真实本地路径。
pub fn import_epub_bytes(
    conn: &Connection,
    library_dir: &Path,
    data: &[u8],
    file_name: Option<&str>,
    now_ms: i64,
) -> Result<ImportOutcome, String> {
    let id = compute_book_id(data);

    if let Some(existing) = get_book(conn, &id).map_err(|e| e.to_string())? {
        return Ok(ImportOutcome {
            book: existing,
            duplicate: true,
        });
    }

    let info = epub_parser::parse_book_info(data)?;

    let objects_dir = library_dir.join("objects");
    std::fs::create_dir_all(&objects_dir).map_err(|e| format!("创建对象仓库失败: {}", e))?;
    let dest = objects_dir.join(format!("{}.epub", id));
    std::fs::write(&dest, data).map_err(|e| format!("写入对象仓库失败: {}", e))?;
    let (cover_path, thumb_path) = save_cover_and_thumb(library_dir, &id, data)?;

    let mut book = LibraryBook {
        id: id.clone(),
        title: if info.metadata.title.trim().is_empty() {
            file_name
                .and_then(|name| {
                    Path::new(name)
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| id.clone())
        } else {
            info.metadata.title
        },
        author: info.metadata.author,
        language: info.metadata.language,
        series: info.metadata.series,
        series_index: info.metadata.series_index,
        description: info.metadata.description,
        file_path: Some(dest.to_string_lossy().into_owned()),
        file_size: Some(data.len() as i64),
        cover_path,
        thumb_path,
        added_at: now_ms,
        last_read_at: None,
        series_id: None,
        volume_id: None,
        edition_id: None,
        availability: None,
        rights_status: Some("user_owned".to_string()),
        remote_url: None,
    };
    // 与 v3 回填/双写同口径填充实体字段，使 ImportOutcome.book 即刻带上它们。
    book.series_id = Some(series_id_of(&book));
    book.volume_id = Some(format!("vol:{}", book.id));
    book.edition_id = Some(format!("ed:{}", book.id));
    book.availability = Some("local".to_string());

    conn.execute(
        "INSERT INTO books
           (id, title, author, language, series, series_index, description,
            file_path, file_size, cover_path, added_at, last_read_at, thumb_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            book.id,
            book.title,
            book.author,
            book.language,
            book.series,
            book.series_index,
            book.description,
            book.file_path,
            book.file_size,
            book.cover_path,
            book.added_at,
            book.last_read_at,
            book.thumb_path
        ],
    )
    .map_err(|e| e.to_string())?;

    // 双写实体链（v0.5-a）：books 仍是读路径，实体表跟着写，保持两侧一致，
    // 为 v0.5-b 切换读路径与远程元数据条目铺路。
    insert_entity_chain(conn, &book).map_err(|e| e.to_string())?;

    Ok(ImportOutcome {
        book,
        duplicate: false,
    })
}

/// 派生某本书的系列 id，与 v3 回填 SQL 同口径：有系列名按名归并，否则按 bookId 独立。
fn series_id_of(book: &LibraryBook) -> String {
    match book.series.as_deref() {
        Some(s) if !s.is_empty() => format!("series:{}", s),
        _ => format!("solo:{}", book.id),
    }
}

/// 导入新书时同步写入 series ← volume ← edition ← asset 实体链。
/// 全程 `INSERT OR IGNORE`：迁移可重入、同系列多卷只补一行 series。
/// `asset.id = book.id`（内容哈希）→ 与 annotations/reading_state 同键。
fn insert_entity_chain(conn: &Connection, book: &LibraryBook) -> rusqlite::Result<()> {
    let series_id = series_id_of(book);
    let volume_id = format!("vol:{}", book.id);
    let edition_id = format!("ed:{}", book.id);
    let series_title = book
        .series
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| book.title.clone());

    conn.execute(
        "INSERT OR IGNORE INTO series(id, title, author, description, cover_path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![
            series_id,
            series_title,
            book.author,
            book.description,
            book.cover_path,
            book.added_at
        ],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO volume(id, series_id, kind, volume_number, title, description, created_at, updated_at)
         VALUES (?1, ?2, 'main', ?3, ?4, ?5, ?6, ?6)",
        params![
            volume_id,
            series_id,
            book.series_index,
            book.title,
            book.description,
            book.added_at
        ],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO edition(id, volume_id, language, rights_status, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'user_owned', ?4, ?4)",
        params![edition_id, volume_id, book.language, book.added_at],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO asset(id, edition_id, kind, availability, file_path, file_size, cover_path, thumb_path, added_at, last_read_at)
         VALUES (?1, ?2, 'epub', 'local', ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            book.id,
            edition_id,
            book.file_path,
            book.file_size,
            book.cover_path,
            book.thumb_path,
            book.added_at,
            book.last_read_at
        ],
    )?;
    Ok(())
}

/// 提取封面原图并生成缩略图。返回 (原图路径, 缩略图路径)；无封面返回 (None, None)。
fn save_cover_and_thumb(
    library_dir: &Path,
    book_id: &str,
    epub_data: &[u8],
) -> Result<(Option<String>, Option<String>), String> {
    let Some(cover) = epub_parser::extract_cover_image(epub_data)? else {
        return Ok((None, None));
    };
    let covers_dir = library_dir.join("covers");
    std::fs::create_dir_all(&covers_dir).map_err(|e| format!("创建封面目录失败: {}", e))?;
    let dest = covers_dir.join(format!("{}.{}", book_id, cover.extension));
    std::fs::write(&dest, &cover.bytes).map_err(|e| format!("写入封面失败: {}", e))?;
    let cover_path = Some(dest.to_string_lossy().into_owned());
    // 缩略图 fail-open：非可解码图片（SVG/webp/损坏）解码或编码失败则跳过，书架回退原图。
    let thumb_path = generate_thumbnail(&covers_dir, book_id, &cover.bytes);
    Ok((cover_path, thumb_path))
}

/// 生成不超过 240×360 的缩略图（保比缩放），统一 PNG 编码。任何失败返回 None。
fn generate_thumbnail(covers_dir: &Path, book_id: &str, cover_bytes: &[u8]) -> Option<String> {
    let img = image::load_from_memory(cover_bytes).ok()?;
    let thumb = img.thumbnail(240, 360);
    let dest = covers_dir.join(format!("{}_thumb.png", book_id));
    thumb
        .save_with_format(&dest, image::ImageFormat::Png)
        .ok()?;
    Some(dest.to_string_lossy().into_owned())
}

// v0.5-c：读路径**锚定 edition**（一个版本 = 书架一个条目），不再读 books。
// 本地条目有 asset（availability=local），远程 metadata_only 条目只有 source_record/无
// asset（file_path/file_size 为 NULL、availability 默认 'remote'），两类都能列出。
// 核心展示字段全部从实体表取，与 v3 回填/导入双写同口径——本地书结果与旧 books 读法等价。
// id：本地 = asset.id（内容哈希，供 open/标注/进度同键）；无 asset 时回退 edition.id。
// series：仅真实系列（id 形如 'series:…'）返回名字，'solo:…' 的孤本仍为 NULL（保旧语义）。
const SELECT_ENTRY: &str = "\
    COALESCE(a.id, e.id) AS id, \
    v.title, \
    s.author, \
    e.language, \
    CASE WHEN s.id LIKE 'series:%' THEN s.title END AS series, \
    v.volume_number, \
    COALESCE(v.description, s.description) AS description, \
    a.file_path, \
    a.file_size, \
    COALESCE(a.cover_path, s.cover_path) AS cover_path, \
    a.thumb_path, \
    COALESCE(a.added_at, 0) AS added_at, \
    a.last_read_at, \
    e.id AS edition_id, \
    v.id AS volume_id, \
    s.id AS series_id, \
    COALESCE(a.availability, 'remote') AS availability, \
    e.rights_status, \
    (SELECT remote_url FROM source_record \
       WHERE entity_type = 'edition' AND entity_id = e.id LIMIT 1) AS remote_url";

const FROM_ENTRY: &str = "\
    FROM edition e \
    JOIN volume v ON v.id = e.volume_id \
    JOIN series s ON s.id = v.series_id \
    LEFT JOIN asset a ON a.edition_id = e.id";

const ORDER_ENTRY: &str = "ORDER BY a.last_read_at IS NULL, a.last_read_at DESC, a.added_at DESC";
const VISIBLE_ENTRY: &str = "\
    (a.id IS NOT NULL OR EXISTS (
      SELECT 1 FROM source_record sr_visible
       WHERE sr_visible.entity_type = 'edition' AND sr_visible.entity_id = e.id
    ))";

fn row_to_book(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryBook> {
    Ok(LibraryBook {
        id: row.get(0)?,
        title: row.get(1)?,
        author: row.get(2)?,
        language: row.get(3)?,
        series: row.get(4)?,
        series_index: row.get(5)?,
        description: row.get(6)?,
        file_path: row.get(7)?,
        file_size: row.get(8)?,
        cover_path: row.get(9)?,
        thumb_path: row.get(10)?,
        added_at: row.get(11)?,
        last_read_at: row.get(12)?,
        edition_id: row.get(13)?,
        volume_id: row.get(14)?,
        series_id: row.get(15)?,
        availability: row.get(16)?,
        rights_status: row.get(17)?,
        remote_url: row.get(18)?,
    })
}

pub fn get_book(conn: &Connection, id: &str) -> rusqlite::Result<Option<LibraryBook>> {
    let sql = format!(
        "SELECT {} {} WHERE COALESCE(a.id, e.id) = ?1 AND {}",
        SELECT_ENTRY, FROM_ENTRY, VISIBLE_ENTRY
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], row_to_book)?;
    rows.next().transpose()
}

/// 全部书目，最近阅读优先、其次最近加入。
pub fn list_books(conn: &Connection) -> rusqlite::Result<Vec<LibraryBook>> {
    let sql = format!(
        "SELECT {} {} WHERE {} {}",
        SELECT_ENTRY, FROM_ENTRY, VISIBLE_ENTRY, ORDER_ENTRY
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_book)?;
    rows.collect()
}

pub fn list_source_records(
    conn: &Connection,
    book_id: &str,
) -> rusqlite::Result<Vec<LibrarySourceRecord>> {
    let Some(edition_id) = source_record_edition_id(conn, book_id)? else {
        return Ok(Vec::new());
    };
    let mut stmt = conn.prepare(
        "SELECT sr.id,
                sr.source_id,
                COALESCE(src.name, sr.source_id) AS source_name,
                COALESCE(src.kind, '') AS source_kind,
                sr.remote_id,
                sr.remote_url,
                sr.rights_status,
                sr.availability,
                sr.last_checked_at
           FROM source_record sr
           LEFT JOIN source src ON src.id = sr.source_id
          WHERE sr.entity_type = 'edition'
            AND sr.entity_id = ?1
          ORDER BY source_name COLLATE NOCASE, sr.source_id, sr.remote_id",
    )?;
    let rows = stmt.query_map([edition_id], |row| {
        Ok(LibrarySourceRecord {
            id: row.get(0)?,
            source_id: row.get(1)?,
            source_name: row.get(2)?,
            source_kind: row.get(3)?,
            remote_id: row.get(4)?,
            remote_url: row.get(5)?,
            rights_status: row.get(6)?,
            availability: row.get(7)?,
            last_checked_at: row.get(8)?,
        })
    })?;
    rows.collect()
}

fn source_record_edition_id(conn: &Connection, id: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT e.id
           FROM edition e
           LEFT JOIN asset a ON a.edition_id = e.id
          WHERE e.id = ?1 OR a.id = ?1
          LIMIT 1",
    )?;
    let mut rows = stmt.query_map([id], |row| row.get(0))?;
    rows.next().transpose()
}

pub fn link_remote_to_local(
    conn: &Connection,
    remote_id: &str,
    local_id: &str,
    now_ms: i64,
) -> Result<LibraryBook, String> {
    if remote_id == local_id {
        return Err("远程条目和本地条目不能相同".to_string());
    }

    let (remote_edition_id, remote_volume_id, remote_series_id) = remote_link_target(conn, remote_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "只能关联尚未本地化的远程元数据条目".to_string())?;
    let (local_asset_id, local_edition_id) = local_link_target(conn, local_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "找不到可关联的本地书库条目".to_string())?;

    if remote_edition_id == local_edition_id {
        return get_book(conn, &local_asset_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "关联后的本地条目丢失".to_string());
    }

    let moved = conn
        .execute(
            "UPDATE source_record
                SET entity_id = ?1, last_checked_at = ?2
              WHERE entity_type = 'edition' AND entity_id = ?3",
            params![local_edition_id, now_ms, remote_edition_id],
        )
        .map_err(|e| e.to_string())?;
    if moved == 0 {
        return Err("远程条目没有可关联的来源记录".to_string());
    }

    // 删除已经没有 asset/source_record 的远程空壳，避免书架和 catalog_fts 继续显示重复条目。
    conn.execute(
        "DELETE FROM edition
          WHERE id = ?1
            AND NOT EXISTS (SELECT 1 FROM asset WHERE edition_id = ?1)
            AND NOT EXISTS (
              SELECT 1 FROM source_record
               WHERE entity_type = 'edition' AND entity_id = ?1
            )",
        params![remote_edition_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM volume
          WHERE id = ?1
            AND NOT EXISTS (SELECT 1 FROM edition WHERE volume_id = ?1)",
        params![remote_volume_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM series
          WHERE id = ?1
            AND NOT EXISTS (SELECT 1 FROM volume WHERE series_id = ?1)",
        params![remote_series_id],
    )
    .map_err(|e| e.to_string())?;

    get_book(conn, &local_asset_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "关联后的本地条目丢失".to_string())
}

fn remote_link_target(
    conn: &Connection,
    id: &str,
) -> rusqlite::Result<Option<(String, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, v.id, s.id
           FROM edition e
           JOIN volume v ON v.id = e.volume_id
           JOIN series s ON s.id = v.series_id
           JOIN source_record sr ON sr.entity_type = 'edition' AND sr.entity_id = e.id
           LEFT JOIN asset a ON a.edition_id = e.id
          WHERE e.id = ?1 OR a.id = ?1
          GROUP BY e.id, v.id, s.id
         HAVING COUNT(a.id) = 0
          LIMIT 1",
    )?;
    let mut rows = stmt.query_map([id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
    rows.next().transpose()
}

fn local_link_target(conn: &Connection, id: &str) -> rusqlite::Result<Option<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, e.id
           FROM asset a
           JOIN edition e ON e.id = a.edition_id
          WHERE (a.id = ?1 OR e.id = ?1)
            AND a.availability IN ('local', 'cached')
          LIMIT 1",
    )?;
    let mut rows = stmt.query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.next().transpose()
}

pub fn remote_acquisition(
    conn: &Connection,
    id: &str,
) -> rusqlite::Result<Option<RemoteAcquisition>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, sr.source_id, sr.remote_id, e.rights_status,
                v.title, s.author, e.language, a.id
           FROM edition e
           JOIN volume v ON v.id = e.volume_id
           JOIN series s ON s.id = v.series_id
           JOIN source_record sr ON sr.entity_type = 'edition' AND sr.entity_id = e.id
           LEFT JOIN asset a ON a.edition_id = e.id
          WHERE e.id = ?1 OR a.id = ?1
          LIMIT 1",
    )?;
    let mut rows = stmt.query_map([id], |row| {
        Ok(RemoteAcquisition {
            edition_id: row.get(0)?,
            source_id: row.get(1)?,
            remote_id: row.get(2)?,
            rights_status: row.get(3)?,
            title: row.get(4)?,
            author: row.get(5)?,
            language: row.get(6)?,
            existing_asset_id: row.get(7)?,
        })
    })?;
    rows.next().transpose()
}

pub fn attach_remote_html_asset(
    conn: &Connection,
    library_dir: &Path,
    edition_id: &str,
    title: &str,
    author: Option<&str>,
    language: Option<&str>,
    source_url: &str,
    html: &str,
    now_ms: i64,
) -> Result<LibraryBook, String> {
    if let Some(existing) =
        existing_asset_for_edition(conn, edition_id).map_err(|e| e.to_string())?
    {
        return get_book(conn, &existing)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "已获取的资产记录丢失".to_string());
    }

    let data = build_single_chapter_epub(title, author, language, source_url, html)?;
    let id = compute_book_id(&data);

    let objects_dir = library_dir.join("objects");
    std::fs::create_dir_all(&objects_dir).map_err(|e| format!("创建对象仓库失败: {e}"))?;
    let dest = objects_dir.join(format!("{}.epub", id));
    std::fs::write(&dest, &data).map_err(|e| format!("写入对象仓库失败: {e}"))?;

    conn.execute(
        "INSERT INTO asset(id, edition_id, kind, availability, file_path, file_size, cover_path, thumb_path, added_at, last_read_at)
         VALUES (?1, ?2, 'epub', 'cached', ?3, ?4, NULL, NULL, ?5, NULL)
         ON CONFLICT(id) DO UPDATE SET
           edition_id = excluded.edition_id,
           availability = excluded.availability,
           file_path = excluded.file_path,
           file_size = excluded.file_size",
        params![
            id,
            edition_id,
            dest.to_string_lossy().into_owned(),
            data.len() as i64,
            now_ms
        ],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE source_record
            SET availability = 'cached', last_checked_at = ?2
          WHERE entity_type = 'edition' AND entity_id = ?1",
        params![edition_id, now_ms],
    )
    .map_err(|e| e.to_string())?;

    get_book(conn, &id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "获取后未能回读书库条目".to_string())
}

/// 将已下载的 EPUB 字节附加到已有的远程元数据 edition 上（OPDS open_license 下载管线）。
/// 与 `attach_remote_html_asset` 不同：输入的已经是标准 EPUB，不需要合成。
pub fn attach_remote_epub_bytes(
    conn: &Connection,
    library_dir: &Path,
    edition_id: &str,
    epub_bytes: &[u8],
    now_ms: i64,
) -> Result<LibraryBook, String> {
    if let Some(existing) =
        existing_asset_for_edition(conn, edition_id).map_err(|e| e.to_string())?
    {
        return get_book(conn, &existing)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "已获取的资产记录丢失".to_string());
    }

    let id = compute_book_id(epub_bytes);

    let objects_dir = library_dir.join("objects");
    std::fs::create_dir_all(&objects_dir).map_err(|e| format!("创建对象仓库失败: {e}"))?;
    let dest = objects_dir.join(format!("{}.epub", id));
    std::fs::write(&dest, epub_bytes).map_err(|e| format!("写入对象仓库失败: {e}"))?;

    // 尝试从 EPUB 提取封面，但 fail-open（远程条目已有 OPDS 提供的封面）。
    let (cover_path, thumb_path) = crate::library::save_cover_and_thumb(library_dir, &id, epub_bytes)
        .unwrap_or((None, None));

    conn.execute(
        "INSERT INTO asset(id, edition_id, kind, availability, file_path, file_size, cover_path, thumb_path, added_at, last_read_at)
         VALUES (?1, ?2, 'epub', 'cached', ?3, ?4, ?5, ?6, ?7, NULL)
         ON CONFLICT(id) DO UPDATE SET
           edition_id = excluded.edition_id,
           availability = excluded.availability,
           file_path = excluded.file_path,
           file_size = excluded.file_size,
           cover_path = COALESCE(excluded.cover_path, asset.cover_path),
           thumb_path = COALESCE(excluded.thumb_path, asset.thumb_path)",
        params![
            id,
            edition_id,
            dest.to_string_lossy().into_owned(),
            epub_bytes.len() as i64,
            cover_path,
            thumb_path,
            now_ms
        ],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE source_record
            SET availability = 'cached', last_checked_at = ?2
          WHERE entity_type = 'edition' AND entity_id = ?1",
        params![edition_id, now_ms],
    )
    .map_err(|e| e.to_string())?;

    get_book(conn, &id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "下载后未能回读书库条目".to_string())
}

fn existing_asset_for_edition(
    conn: &Connection,
    edition_id: &str,
) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT id FROM asset WHERE edition_id = ?1 LIMIT 1")?;
    let mut rows = stmt.query_map([edition_id], |row| row.get(0))?;
    rows.next().transpose()
}

fn build_single_chapter_epub(
    title: &str,
    author: Option<&str>,
    language: Option<&str>,
    source_url: &str,
    html: &str,
) -> Result<Vec<u8>, String> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    writer
        .start_file("mimetype", stored)
        .map_err(|e| e.to_string())?;
    writer
        .write_all(b"application/epub+zip")
        .map_err(|e| e.to_string())?;

    writer
        .start_file("META-INF/container.xml", deflated)
        .map_err(|e| e.to_string())?;
    writer
        .write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
        )
        .map_err(|e| e.to_string())?;

    let title_xml = escape_xml(title);
    let language_xml = escape_xml(language.unwrap_or("ja"));
    let author_xml = author.map(escape_xml);
    let creator = author_xml
        .as_ref()
        .map(|a| format!("\n    <dc:creator>{}</dc:creator>", a))
        .unwrap_or_default();
    let source_xml = escape_xml(source_url);
    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="bookid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">{}</dc:identifier>
    <dc:title>{}</dc:title>{}
    <dc:language>{}</dc:language>
    <dc:source>{}</dc:source>
  </metadata>
  <manifest>
    <item id="chap1" href="Text/chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chap1"/>
  </spine>
</package>"#,
        stable_generated_id(source_url, title),
        title_xml,
        creator,
        language_xml,
        source_xml,
    );
    writer
        .start_file("OEBPS/content.opf", deflated)
        .map_err(|e| e.to_string())?;
    writer
        .write_all(opf.as_bytes())
        .map_err(|e| e.to_string())?;

    writer
        .start_file("OEBPS/Text/chapter1.xhtml", deflated)
        .map_err(|e| e.to_string())?;
    writer
        .write_all(html.as_bytes())
        .map_err(|e| e.to_string())?;

    writer
        .finish()
        .map(|c| c.into_inner())
        .map_err(|e| e.to_string())
}

fn stable_generated_id(source_url: &str, title: &str) -> String {
    let seed = format!("aozora-html\0{}\0{}", source_url, title);
    format!("generated:{}", compute_book_id(seed.as_bytes()))
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// 标题/作者/系列搜索。≥3 字走 FTS5 trigram(子串命中),否则 LIKE 兜底。
pub fn search_books(conn: &Connection, query: &str) -> rusqlite::Result<Vec<LibraryBook>> {
    let q = query.trim();
    if q.is_empty() {
        return list_books(conn);
    }

    if q.chars().count() >= 3 {
        // ≥3 字走 catalog_fts（trigram 子串命中）。catalog_fts 由实体表同步，
        // 因此本地 asset 与远程 metadata-only 条目都能命中。
        let phrase = format!("\"{}\"", q.replace('"', "\"\""));
        let sql = format!(
            "SELECT {} {}
              JOIN catalog_fts ON catalog_fts.edition_id = e.id
             WHERE catalog_fts MATCH ?1
               AND {}
             ORDER BY rank",
            SELECT_ENTRY, FROM_ENTRY, VISIBLE_ENTRY
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([phrase], row_to_book)?;
        return rows.collect();
    }

    // 短查询:LIKE 子串,通配符转义。直接打实体表 → 本地与远程条目都能命中。
    let escaped = q
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pattern = format!("%{}%", escaped);
    let sql = format!(
        "SELECT {} {}
          WHERE (
                v.title LIKE ?1 ESCAPE '\\'
             OR s.author LIKE ?1 ESCAPE '\\'
             OR (s.id LIKE 'series:%' AND s.title LIKE ?1 ESCAPE '\\')
          )
            AND {}
          {}",
        SELECT_ENTRY, FROM_ENTRY, VISIBLE_ENTRY, ORDER_ENTRY
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([pattern], row_to_book)?;
    rows.collect()
}

/// 更新最近阅读时间。读路径读 `asset.last_read_at`，故必须更 asset；books 作为
/// 只读镜像一并更新，保持两侧一致。`id` 为内容哈希（= 本地 asset.id）。
pub fn touch_last_read(conn: &Connection, id: &str, ts_ms: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE asset SET last_read_at = ?2 WHERE id = ?1",
        params![id, ts_ms],
    )?;
    conn.execute(
        "UPDATE books SET last_read_at = ?2 WHERE id = ?1",
        params![id, ts_ms],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    /// 构造最小合法 EPUB(container.xml + OPF + 单章;TOC 由 spine 兜底)。
    fn make_epub(title: &str, author: &str) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        // 固定时间戳：默认会写入当前时间，致同内容 epub 跨秒字节不同、内容哈希漂移（去重 flaky）。
        let opts = SimpleFileOptions::default().last_modified_time(zip::DateTime::default());
        w.start_file("mimetype", opts).unwrap();
        w.write_all(b"application/epub+zip").unwrap();
        w.start_file("META-INF/container.xml", opts).unwrap();
        w.write_all(
            br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
        )
        .unwrap();
        w.start_file("content.opf", opts).unwrap();
        w.write_all(
            format!(
                r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="uid">test-{title}</dc:identifier>
    <dc:title>{title}</dc:title>
    <dc:creator>{author}</dc:creator>
  </metadata>
  <manifest>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#
            )
            .as_bytes(),
        )
        .unwrap();
        w.start_file("ch1.xhtml", opts).unwrap();
        w.write_all(b"<html><body><p>text</p></body></html>")
            .unwrap();
        w.finish().unwrap().into_inner()
    }

    fn make_epub_with_cover(title: &str, author: &str) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        // 固定时间戳：默认会写入当前时间，致同内容 epub 跨秒字节不同、内容哈希漂移（去重 flaky）。
        let opts = SimpleFileOptions::default().last_modified_time(zip::DateTime::default());
        w.start_file("mimetype", opts).unwrap();
        w.write_all(b"application/epub+zip").unwrap();
        w.start_file("META-INF/container.xml", opts).unwrap();
        w.write_all(
            br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
        )
        .unwrap();
        w.start_file("OEBPS/content.opf", opts).unwrap();
        w.write_all(
            format!(
                r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="uid">test-{title}</dc:identifier>
    <dc:title>{title}</dc:title>
    <dc:creator>{author}</dc:creator>
  </metadata>
  <manifest>
    <item id="cover-img" href="Images/cover.png" media-type="image/png" properties="cover-image"/>
    <item id="ch1" href="Text/ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#
            )
            .as_bytes(),
        )
        .unwrap();
        w.start_file("OEBPS/Images/cover.png", opts).unwrap();
        w.write_all(b"fake-png-cover").unwrap();
        w.start_file("OEBPS/Text/ch1.xhtml", opts).unwrap();
        w.write_all(b"<html><body><p>text</p></body></html>")
            .unwrap();
        w.finish().unwrap().into_inner()
    }

    fn make_epub_with_rich_metadata(title: &str, author: &str) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        // 固定时间戳：默认会写入当前时间，致同内容 epub 跨秒字节不同、内容哈希漂移（去重 flaky）。
        let opts = SimpleFileOptions::default().last_modified_time(zip::DateTime::default());
        w.start_file("mimetype", opts).unwrap();
        w.write_all(b"application/epub+zip").unwrap();
        w.start_file("META-INF/container.xml", opts).unwrap();
        w.write_all(
            br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
        )
        .unwrap();
        w.start_file("content.opf", opts).unwrap();
        w.write_all(
            format!(
                r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="uid">test-rich-{title}</dc:identifier>
    <dc:title>{title}</dc:title>
    <dc:creator>{author}</dc:creator>
    <dc:language>ja</dc:language>
    <dc:description>First volume description</dc:description>
    <meta property="belongs-to-collection">Skyline Chronicle</meta>
    <meta property="group-position">2.5</meta>
  </metadata>
  <manifest>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#
            )
            .as_bytes(),
        )
        .unwrap();
        w.start_file("ch1.xhtml", opts).unwrap();
        w.write_all(b"<html><body><p>text</p></body></html>")
            .unwrap();
        w.finish().unwrap().into_inner()
    }

    fn temp_library(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "reading-core-libtest-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn import_fixture(
        conn: &Connection,
        dir: &Path,
        name: &str,
        title: &str,
        author: &str,
    ) -> ImportOutcome {
        let path = dir.join(name);
        std::fs::write(&path, make_epub(title, author)).unwrap();
        import_epub(conn, dir, &path, 1000).unwrap()
    }

    #[test]
    fn import_dedupe_and_object_store() {
        let dir = temp_library("import");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();

        let r1 = import_fixture(&conn, &dir, "a.epub", "凉宫春日的忧郁", "谷川流");
        assert!(!r1.duplicate);
        assert_eq!(r1.book.title, "凉宫春日的忧郁");
        assert_eq!(r1.book.author.as_deref(), Some("谷川流"));
        assert!(std::path::Path::new(r1.book.file_path.as_deref().unwrap()).exists());
        assert_eq!(r1.book.id.len(), 32);

        // 同内容不同文件名 → 去重命中
        let dup_path = dir.join("b.epub");
        std::fs::write(&dup_path, make_epub("凉宫春日的忧郁", "谷川流")).unwrap();
        let r2 = import_epub(&conn, &dir, &dup_path, 2000).unwrap();
        assert!(r2.duplicate);
        assert_eq!(r2.book.id, r1.book.id);
        assert_eq!(list_books(&conn).unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_bytes_uses_filename_when_title_missing() {
        let dir = temp_library("import-bytes");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();
        let data = make_epub("", "作者");

        let result =
            import_epub_bytes(&conn, &dir, &data, Some("fallback-title.epub"), 1000).unwrap();

        assert_eq!(result.book.title, "fallback-title");
        assert!(!result.duplicate);
        assert!(std::path::Path::new(result.book.file_path.as_deref().unwrap()).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_extracts_cover_image() {
        let dir = temp_library("cover");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();
        let data = make_epub_with_cover("有封面的书", "作者");

        let result = import_epub_bytes(&conn, &dir, &data, Some("covered.epub"), 1000).unwrap();

        let cover_path = result.book.cover_path.expect("应提取封面");
        assert!(cover_path.ends_with(".png"));
        assert_eq!(std::fs::read(&cover_path).unwrap(), b"fake-png-cover");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_extracts_rich_metadata() {
        let dir = temp_library("metadata");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();
        let data = make_epub_with_rich_metadata("Rich Book", "Metadata Author");

        let result = import_epub_bytes(&conn, &dir, &data, Some("rich.epub"), 1000).unwrap();

        assert_eq!(result.book.language.as_deref(), Some("ja"));
        assert_eq!(
            result.book.description.as_deref(),
            Some("First volume description")
        );
        assert_eq!(result.book.series.as_deref(), Some("Skyline Chronicle"));
        assert_eq!(result.book.series_index, Some(2.5));

        let hit = search_books(&conn, "Skyline").unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].id, result.book.id);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_fts_trigram_and_like_fallback() {
        let dir = temp_library("search");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();
        import_fixture(&conn, &dir, "a.epub", "凉宫春日的忧郁", "谷川流");
        import_fixture(&conn, &dir, "b.epub", "文学少女", "野村美月");

        // ≥3 字:FTS trigram 子串命中(标题中段)
        let hit = search_books(&conn, "春日的忧").unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].title, "凉宫春日的忧郁");

        // 作者命中
        let hit = search_books(&conn, "野村美月").unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].title, "文学少女");

        // <3 字:LIKE 兜底
        let hit = search_books(&conn, "春日").unwrap();
        assert_eq!(hit.len(), 1);

        // 未命中
        assert!(search_books(&conn, "不存在的书").unwrap().is_empty());

        // 空查询 = 全部
        assert_eq!(search_books(&conn, "  ").unwrap().len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_library_stamps_schema_version() {
        let dir = temp_library("version");
        let path = dir.join("library.sqlite");

        let conn = open_library(&path).unwrap();
        assert_eq!(crate::migrations::current_version(&conn).unwrap(), 5);
        drop(conn);

        // 重开已有库：迁移幂等跳过，版本不变、数据仍在。
        let conn = open_library(&path).unwrap();
        assert_eq!(crate::migrations::current_version(&conn).unwrap(), 5);

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn count_rows(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn import_double_writes_entity_chain() {
        let dir = temp_library("entity-dw");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();
        let r = import_fixture(&conn, &dir, "a.epub", "孤本书", "作者甲");

        // asset.id == book.id（内容哈希）→ 标注/进度同键。
        let asset_id: String = conn
            .query_row("SELECT id FROM asset WHERE id = ?1", [&r.book.id], |x| {
                x.get(0)
            })
            .unwrap();
        assert_eq!(asset_id, r.book.id);

        // 四层链各一行。
        assert_eq!(count_rows(&conn, "series"), 1);
        assert_eq!(count_rows(&conn, "volume"), 1);
        assert_eq!(count_rows(&conn, "edition"), 1);
        assert_eq!(count_rows(&conn, "asset"), 1);

        // asset → edition → volume → series 能串起来；无系列名时 series.title 回退书名。
        let series_title: String = conn
            .query_row(
                "SELECT s.title FROM asset a
                   JOIN edition e ON e.id = a.edition_id
                   JOIN volume  v ON v.id = e.volume_id
                   JOIN series  s ON s.id = v.series_id
                  WHERE a.id = ?1",
                [&r.book.id],
                |x| x.get(0),
            )
            .unwrap();
        assert_eq!(series_title, "孤本书");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn same_series_books_share_one_series_row() {
        let dir = temp_library("entity-series");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();
        // rich metadata fixture 固定 series = "Skyline Chronicle"；不同标题 → 不同内容哈希。
        let a = import_epub_bytes(
            &conn,
            &dir,
            &make_epub_with_rich_metadata("卷一", "作者"),
            Some("v1.epub"),
            1000,
        )
        .unwrap();
        let b = import_epub_bytes(
            &conn,
            &dir,
            &make_epub_with_rich_metadata("卷二", "作者"),
            Some("v2.epub"),
            2000,
        )
        .unwrap();
        assert_ne!(a.book.id, b.book.id);

        assert_eq!(
            count_rows(&conn, "series"),
            1,
            "同系列两卷应共用一个 series"
        );
        assert_eq!(count_rows(&conn, "volume"), 2);
        assert_eq!(count_rows(&conn, "asset"), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migration_v3_backfills_preexisting_books() {
        // 模拟升级前的 v2 旧库：跑到 v2、直接塞一行 books，再 open_library 触发 v3 回填。
        let dir = temp_library("entity-backfill");
        let path = dir.join("library.sqlite");
        {
            let conn = Connection::open(&path).unwrap();
            migrations::run(&conn, &MIGRATIONS[..2]).unwrap();
            assert_eq!(migrations::current_version(&conn).unwrap(), 2);
            conn.execute(
                "INSERT INTO books
                   (id, title, author, language, series, series_index, description,
                    file_path, file_size, cover_path, added_at, last_read_at, thumb_path)
                 VALUES ('hash123', '旧书', '旧作者', 'zh', '旧系列', 1.0, 'desc',
                         '/p.epub', 10, NULL, 500, NULL, NULL)",
                [],
            )
            .unwrap();
        }

        // 重开 → 跑 v3-v5，回填实体链并建立 catalog_fts。
        let conn = open_library(&path).unwrap();
        assert_eq!(migrations::current_version(&conn).unwrap(), 5);

        let asset_id: String = conn
            .query_row("SELECT id FROM asset WHERE id = 'hash123'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(asset_id, "hash123");

        let series_id: String = conn
            .query_row(
                "SELECT series_id FROM volume WHERE id = 'vol:hash123'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(series_id, "series:旧系列");

        // 再次 open 幂等：回填不重复。
        drop(conn);
        let conn = open_library(&path).unwrap();
        assert_eq!(count_rows(&conn, "asset"), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migration_v5_backfills_remote_entries_into_catalog_fts() {
        // 模拟 v4 旧库：已有远程 metadata-only 实体，但 catalog_fts 仍是 v3 预建空表。
        let dir = temp_library("catalog-fts-backfill");
        let path = dir.join("library.sqlite");
        {
            let conn = Connection::open(&path).unwrap();
            migrations::run(&conn, &MIGRATIONS[..4]).unwrap();
            assert_eq!(migrations::current_version(&conn).unwrap(), 4);
            conn.execute_batch(
                "INSERT INTO series(id,title,author,description,created_at,updated_at)
                   VALUES('series:remote-old','旧远程系列','旧远程作者',NULL,0,0);
                 INSERT INTO volume(id,series_id,kind,volume_number,title,created_at,updated_at)
                   VALUES('vol:remote-old','series:remote-old','main',NULL,'旧远程标题',0,0);
                 INSERT INTO edition(id,volume_id,language,rights_status,created_at,updated_at)
                   VALUES('ed:remote-old','vol:remote-old','ja','unknown',0,0);
                 INSERT INTO source(id,name,kind,created_at)
                   VALUES('src:old','Old Source','metadata',0);
                 INSERT INTO source_record(id,source_id,entity_type,entity_id,remote_url,rights_status,availability)
                   VALUES('sr:remote-old','src:old','edition','ed:remote-old',
                          'https://example/old','unknown','remote');",
            )
            .unwrap();
        }

        let conn = open_library(&path).unwrap();
        assert_eq!(migrations::current_version(&conn).unwrap(), 5);
        let hit = search_books(&conn, "旧远程标题").unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].id, "ed:remote-old");
        assert_eq!(hit[0].availability.as_deref(), Some("remote"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn queries_join_backfill_entity_fields() {
        let dir = temp_library("entity-read");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();
        let r = import_epub_bytes(
            &conn,
            &dir,
            &make_epub_with_rich_metadata("卷一", "作者"),
            Some("v1.epub"),
            1000,
        )
        .unwrap();

        // import 返回值即刻带实体字段。
        assert_eq!(r.book.availability.as_deref(), Some("local"));
        assert_eq!(
            r.book.series_id.as_deref(),
            Some("series:Skyline Chronicle")
        );
        assert_eq!(r.book.volume_id, Some(format!("vol:{}", r.book.id)));
        assert_eq!(r.book.edition_id, Some(format!("ed:{}", r.book.id)));

        // get_book 经 JOIN 回填同样字段。
        let got = get_book(&conn, &r.book.id).unwrap().unwrap();
        assert_eq!(got.series_id.as_deref(), Some("series:Skyline Chronicle"));
        assert_eq!(got.volume_id, Some(format!("vol:{}", r.book.id)));
        assert_eq!(got.edition_id, Some(format!("ed:{}", r.book.id)));
        assert_eq!(got.availability.as_deref(), Some("local"));
        // 核心字段不受 JOIN 影响。
        assert_eq!(got.series.as_deref(), Some("Skyline Chronicle"));
        assert_eq!(got.series_index, Some(2.5));

        // list / search 路径同样回填。
        let listed = list_books(&conn).unwrap();
        assert_eq!(listed[0].edition_id, Some(format!("ed:{}", r.book.id)));
        let hit = search_books(&conn, "Skyline").unwrap();
        assert_eq!(hit[0].availability.as_deref(), Some("local"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remote_metadata_only_entry_is_listable() {
        // 读路径锚定 edition 的核心收益：无 asset 的远程条目（连接器产出）也能上书架。
        let dir = temp_library("entity-remote");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();
        let local = import_fixture(&conn, &dir, "local.epub", "本地书", "作者");

        // 手工塞一个只有 series/volume/edition + source_record、无 asset 的远程条目。
        conn.execute_batch(
            "INSERT INTO series(id,title,author,description,created_at,updated_at)
               VALUES('series:远程系列','远程系列','远程作者','简介',0,0);
             INSERT INTO volume(id,series_id,kind,volume_number,title,created_at,updated_at)
               VALUES('vol:remote1','series:远程系列','main',1,'远程卷一',0,0);
             INSERT INTO edition(id,volume_id,language,rights_status,created_at,updated_at)
               VALUES('ed:remote1','vol:remote1','ja','official_purchase',0,0);
             INSERT INTO source(id,name,kind,created_at)
               VALUES('src:anilist','AniList','metadata',0);
             INSERT INTO source_record(id,source_id,entity_type,entity_id,remote_url,rights_status)
               VALUES('sr:1','src:anilist','edition','ed:remote1','https://example/x','official_purchase');",
        )
        .unwrap();

        let books = list_books(&conn).unwrap();
        assert_eq!(books.len(), 2, "本地 + 远程两个条目都应在书架");

        let remote = books
            .iter()
            .find(|b| b.id == "ed:remote1")
            .expect("远程条目应出现在书架");
        assert_eq!(remote.availability.as_deref(), Some("remote"));
        assert_eq!(remote.rights_status.as_deref(), Some("official_purchase"));
        assert!(remote.file_path.is_none(), "远程条目无本地文件");
        assert!(remote.file_size.is_none());
        assert_eq!(remote.series.as_deref(), Some("远程系列"));
        assert_eq!(remote.title, "远程卷一");
        assert_eq!(remote.edition_id.as_deref(), Some("ed:remote1"));

        // get_book 按 edition id 也能取到远程条目。
        let got = get_book(&conn, "ed:remote1")
            .unwrap()
            .expect("应能按 edition id 取远程条目");
        assert_eq!(got.id, "ed:remote1");

        // ≥3 字搜索现在走 catalog_fts，远程 metadata-only 条目也应被标题/作者/系列命中。
        let hit = search_books(&conn, "远程卷一").unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].id, "ed:remote1");
        assert_eq!(hit[0].availability.as_deref(), Some("remote"));

        let hit = search_books(&conn, "远程作者").unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].id, "ed:remote1");

        let hit = search_books(&conn, "远程系列").unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].id, "ed:remote1");

        conn.execute(
            "UPDATE volume SET title = '全新修订版' WHERE id = 'vol:remote1'",
            [],
        )
        .unwrap();
        assert!(search_books(&conn, "远程卷一").unwrap().is_empty());
        let hit = search_books(&conn, "修订版").unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].id, "ed:remote1");

        conn.execute(
            "UPDATE series SET author = '远程作者改' WHERE id = 'series:远程系列'",
            [],
        )
        .unwrap();
        let hit = search_books(&conn, "作者改").unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].id, "ed:remote1");

        // 本地书仍按内容哈希取到、且有文件。
        let got_local = get_book(&conn, &local.book.id).unwrap().unwrap();
        assert!(got_local.file_path.is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn link_remote_metadata_entry_to_local_asset_hides_remote_shell() {
        let dir = temp_library("remote-link");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();
        let local = import_fixture(&conn, &dir, "local.epub", "Local Volume", "Local Author");
        let local_edition_id = local.book.edition_id.clone().unwrap();

        conn.execute_batch(
            "INSERT INTO series(id,title,author,description,created_at,updated_at)
               VALUES('series:remote-link','Remote Series','Remote Author','Remote description',0,0);
             INSERT INTO volume(id,series_id,kind,volume_number,title,created_at,updated_at)
               VALUES('vol:remote-link','series:remote-link','main',1,'Remote Volume',0,0);
             INSERT INTO edition(id,volume_id,language,rights_status,created_at,updated_at)
               VALUES('ed:remote-link','vol:remote-link','ja','official_purchase',0,0);
             INSERT INTO source(id,name,kind,created_at)
               VALUES('src:test','Test Source','metadata',0);
             INSERT INTO source_record(id,source_id,entity_type,entity_id,remote_url,remote_id,rights_status)
               VALUES('sr:remote-link','src:test','edition','ed:remote-link','https://example.test/remote','remote-1','official_purchase');",
        )
        .unwrap();

        assert_eq!(list_books(&conn).unwrap().len(), 2);
        assert_eq!(
            get_book(&conn, "ed:remote-link")
                .unwrap()
                .unwrap()
                .availability
                .as_deref(),
            Some("remote")
        );
        assert_eq!(search_books(&conn, "Remote").unwrap().len(), 1);

        let remote_records = list_source_records(&conn, "ed:remote-link").unwrap();
        assert_eq!(remote_records.len(), 1);
        assert_eq!(remote_records[0].source_id, "src:test");
        assert_eq!(remote_records[0].source_name, "Test Source");
        assert_eq!(remote_records[0].source_kind, "metadata");
        assert_eq!(
            remote_records[0].remote_url.as_deref(),
            Some("https://example.test/remote")
        );
        assert_eq!(remote_records[0].remote_id.as_deref(), Some("remote-1"));

        let linked = link_remote_to_local(&conn, "ed:remote-link", &local.book.id, 2000).unwrap();
        assert_eq!(linked.id, local.book.id);
        assert_eq!(linked.edition_id.as_deref(), Some(local_edition_id.as_str()));
        assert_eq!(linked.availability.as_deref(), Some("local"));

        let books = list_books(&conn).unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, local.book.id);
        assert!(get_book(&conn, "ed:remote-link").unwrap().is_none());
        assert!(search_books(&conn, "Remote").unwrap().is_empty());

        let moved_entity: String = conn
            .query_row(
                "SELECT entity_id FROM source_record WHERE id = 'sr:remote-link'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(moved_entity, local_edition_id);

        assert!(list_source_records(&conn, "ed:remote-link")
            .unwrap()
            .is_empty());
        let local_records = list_source_records(&conn, &local.book.id).unwrap();
        assert_eq!(local_records.len(), 1);
        assert_eq!(local_records[0].source_id, "src:test");
        assert_eq!(local_records[0].rights_status, "official_purchase");
        assert_eq!(local_records[0].last_checked_at, Some(2000));
        let local_records_by_edition =
            list_source_records(&conn, linked.edition_id.as_deref().unwrap()).unwrap();
        assert_eq!(local_records_by_edition.len(), 1);
        assert_eq!(local_records_by_edition[0].id, "sr:remote-link");

        // 模拟同一远程搜索再次写出 edition 空壳，但 source_record 已经归到本地 edition。
        // 这种无 asset 且无 source_record 的空壳不应重新出现在书架或搜索结果里。
        conn.execute_batch(
            "INSERT INTO series(id,title,author,created_at,updated_at)
               VALUES('series:remote-link','Remote Series','Remote Author',0,0);
             INSERT INTO volume(id,series_id,kind,volume_number,title,created_at,updated_at)
               VALUES('vol:remote-link','series:remote-link','main',1,'Remote Volume',0,0);
             INSERT INTO edition(id,volume_id,language,rights_status,created_at,updated_at)
               VALUES('ed:remote-link','vol:remote-link','ja','official_purchase',0,0);",
        )
        .unwrap();

        let books = list_books(&conn).unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, local.book.id);
        assert!(get_book(&conn, "ed:remote-link").unwrap().is_none());
        assert!(search_books(&conn, "Remote").unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn public_domain_remote_html_attaches_cached_asset_to_existing_edition() {
        let dir = temp_library("acquire-html");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();

        conn.execute_batch(
            "INSERT INTO series(id,title,author,description,created_at,updated_at)
               VALUES('series:aozora:127','羅生門','芥川龍之介',NULL,0,0);
             INSERT INTO volume(id,series_id,kind,volume_number,title,created_at,updated_at)
               VALUES('vol:aozora:127','series:aozora:127','main',NULL,'羅生門',0,0);
             INSERT INTO edition(id,volume_id,language,rights_status,created_at,updated_at)
               VALUES('ed:aozora:127','vol:aozora:127','ja','public_domain',0,0);
             INSERT INTO source(id,name,kind,created_at)
               VALUES('src:aozora','青空文庫','catalog',0);
             INSERT INTO source_record(id,source_id,entity_type,entity_id,remote_url,remote_id,rights_status,availability)
               VALUES('sr:aozora:127','src:aozora','edition','ed:aozora:127',
                      'https://www.aozora.gr.jp/cards/000879/card127.html','127','public_domain','remote');",
        )
        .unwrap();

        let info = remote_acquisition(&conn, "ed:aozora:127")
            .unwrap()
            .expect("应可获取远程来源信息");
        assert_eq!(info.source_id, "src:aozora");
        assert_eq!(info.rights_status, "public_domain");
        assert!(info.existing_asset_id.is_none());

        let html = r#"<!doctype html><html><body><h1>羅生門</h1><p>ある日の暮方の事である。</p></body></html>"#;
        let book = attach_remote_html_asset(
            &conn,
            &dir,
            "ed:aozora:127",
            "羅生門",
            Some("芥川龍之介"),
            Some("ja"),
            "https://www.aozora.gr.jp/cards/000879/files/127_15260.html",
            html,
            2000,
        )
        .unwrap();

        assert_eq!(book.availability.as_deref(), Some("cached"));
        assert_eq!(book.rights_status.as_deref(), Some("public_domain"));
        assert_eq!(book.edition_id.as_deref(), Some("ed:aozora:127"));
        assert!(book
            .file_path
            .as_ref()
            .is_some_and(|p| Path::new(p).exists()));

        let availability: String = conn
            .query_row(
                "SELECT availability FROM source_record WHERE id='sr:aozora:127'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(availability, "cached");

        let bytes = std::fs::read(book.file_path.as_ref().unwrap()).unwrap();
        let info = epub_parser::parse_book_info(&bytes).unwrap();
        assert_eq!(info.metadata.title, "羅生門");
        let chapter =
            epub_parser::parse_single_chapter(&bytes, &info.spine[0].href, &info).unwrap();
        assert!(chapter.contains("ある日の暮方"));

        // 再次 attach 不重复生成 asset，直接回读既有条目。
        let again = attach_remote_html_asset(
            &conn,
            &dir,
            "ed:aozora:127",
            "羅生門",
            Some("芥川龍之介"),
            Some("ja"),
            "https://www.aozora.gr.jp/cards/000879/files/127_15260.html",
            html,
            3000,
        )
        .unwrap();
        assert_eq!(again.id, book.id);

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn make_epub_with_cover_bytes(title: &str, cover: &[u8]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        // 固定时间戳：默认会写入当前时间，致同内容 epub 跨秒字节不同、内容哈希漂移（去重 flaky）。
        let opts = SimpleFileOptions::default().last_modified_time(zip::DateTime::default());
        w.start_file("mimetype", opts).unwrap();
        w.write_all(b"application/epub+zip").unwrap();
        w.start_file("META-INF/container.xml", opts).unwrap();
        w.write_all(
            br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#,
        )
        .unwrap();
        w.start_file("OEBPS/content.opf", opts).unwrap();
        w.write_all(
            format!(
                r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="uid">u-{title}</dc:identifier><dc:title>{title}</dc:title></metadata>
  <manifest>
    <item id="cover-img" href="Images/cover.png" media-type="image/png" properties="cover-image"/>
    <item id="ch1" href="Text/ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="ch1"/></spine>
</package>"#
            )
            .as_bytes(),
        )
        .unwrap();
        w.start_file("OEBPS/Images/cover.png", opts).unwrap();
        w.write_all(cover).unwrap();
        w.start_file("OEBPS/Text/ch1.xhtml", opts).unwrap();
        w.write_all(b"<html><body><p>text</p></body></html>")
            .unwrap();
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn import_generates_thumbnail_from_real_cover() {
        let dir = temp_library("thumb");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();
        // 用 image 生成一张真实 300×450 PNG 作封面
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            300,
            450,
            image::Rgb([120, 90, 160]),
        ))
        .write_to(&mut buf, image::ImageFormat::Png)
        .unwrap();
        let data = make_epub_with_cover_bytes("有真封面", &buf.into_inner());

        let result = import_epub_bytes(&conn, &dir, &data, Some("c.epub"), 1000).unwrap();

        let thumb = result.book.thumb_path.clone().expect("应生成缩略图");
        assert!(thumb.ends_with("_thumb.png"));
        let decoded = image::open(&thumb).expect("缩略图应为有效 PNG");
        assert!(
            decoded.width() <= 240 && decoded.height() <= 360,
            "缩略图应缩到 240×360 内: {}x{}",
            decoded.width(),
            decoded.height()
        );
        // 读取回来 thumb_path 仍在（迁移 v2 列 + row 映射正确）
        let got = get_book(&conn, &result.book.id).unwrap().unwrap();
        assert_eq!(got.thumb_path, result.book.thumb_path);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_with_undecodable_cover_skips_thumbnail() {
        let dir = temp_library("thumb-none");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();
        // 假 PNG（不可解码）：封面原图仍保存，缩略图 fail-open 跳过
        let data = make_epub_with_cover("假封面", "作者");
        let result = import_epub_bytes(&conn, &dir, &data, Some("f.epub"), 1000).unwrap();
        assert!(result.book.cover_path.is_some(), "封面原图仍应保存");
        assert!(result.book.thumb_path.is_none(), "不可解码封面应跳过缩略图");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn last_read_ordering() {
        let dir = temp_library("order");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();
        let a = import_fixture(&conn, &dir, "a.epub", "书甲", "甲");
        let _b = import_fixture(&conn, &dir, "b.epub", "书乙", "乙");

        touch_last_read(&conn, &a.book.id, 9999).unwrap();
        let books = list_books(&conn).unwrap();
        assert_eq!(books[0].title, "书甲"); // 最近读过的排最前

        let _ = std::fs::remove_dir_all(&dir);
    }
}
