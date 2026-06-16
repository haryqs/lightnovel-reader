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
use std::path::Path;

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
    /// 库内对象路径（objects/<id>.epub）。
    pub file_path: String,
    pub file_size: i64,
    pub cover_path: Option<String>,
    /// 小尺寸缩略图（covers/<id>_thumb.png），书架优先加载它而非原图。无则为 None。
    pub thumb_path: Option<String>,
    pub added_at: i64,
    pub last_read_at: Option<i64>,
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

/// 书库数据库的迁移序列。新增列/表一律追加新版本，绝不改 SCHEMA_V1
/// （旧库已盖戳 v1，不会再跑 v1）。v0.5 实体模型（series/volume/edition/asset）作为更高版本追加。
const MIGRATIONS: &[Migration] = &[
    Migration { version: 1, sql: SCHEMA_V1 },
    // v2：封面缩略图列。已在 v1 的旧库经 ALTER 补列；新库 v1 建表后 v2 补列。
    Migration { version: 2, sql: "ALTER TABLE books ADD COLUMN thumb_path TEXT;" },
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

    let book = LibraryBook {
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
        file_path: dest.to_string_lossy().into_owned(),
        file_size: data.len() as i64,
        cover_path,
        thumb_path,
        added_at: now_ms,
        last_read_at: None,
    };

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

    Ok(ImportOutcome {
        book,
        duplicate: false,
    })
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
    thumb.save_with_format(&dest, image::ImageFormat::Png).ok()?;
    Some(dest.to_string_lossy().into_owned())
}

const BOOK_COLS: &str = "id, title, author, language, series, series_index, description,
                         file_path, file_size, cover_path, added_at, last_read_at, thumb_path";

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
        added_at: row.get(10)?,
        last_read_at: row.get(11)?,
        thumb_path: row.get(12)?,
    })
}

pub fn get_book(conn: &Connection, id: &str) -> rusqlite::Result<Option<LibraryBook>> {
    let sql = format!("SELECT {} FROM books WHERE id = ?1", BOOK_COLS);
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], row_to_book)?;
    rows.next().transpose()
}

/// 全部书目，最近阅读优先、其次最近加入。
pub fn list_books(conn: &Connection) -> rusqlite::Result<Vec<LibraryBook>> {
    let sql = format!(
        "SELECT {} FROM books
          ORDER BY last_read_at IS NULL, last_read_at DESC, added_at DESC",
        BOOK_COLS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_book)?;
    rows.collect()
}

/// 标题/作者/系列搜索。≥3 字走 FTS5 trigram(子串命中),否则 LIKE 兜底。
pub fn search_books(conn: &Connection, query: &str) -> rusqlite::Result<Vec<LibraryBook>> {
    let q = query.trim();
    if q.is_empty() {
        return list_books(conn);
    }

    if q.chars().count() >= 3 {
        // 引号包裹成短语字面量,内部引号按 FTS 规则翻倍转义
        let phrase = format!("\"{}\"", q.replace('"', "\"\""));
        // books_fts 同名列会造成歧义,全部列加表前缀
        let cols: Vec<String> = BOOK_COLS
            .split(',')
            .map(|c| format!("b.{}", c.trim()))
            .collect();
        let sql = format!(
            "SELECT {} FROM books b
              JOIN books_fts f ON f.rowid = b.rowid
             WHERE books_fts MATCH ?1
             ORDER BY rank",
            cols.join(", ")
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([phrase], row_to_book)?;
        return rows.collect();
    }

    // 短查询:LIKE 子串,通配符转义
    let escaped = q
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pattern = format!("%{}%", escaped);
    let sql = format!(
        "SELECT {} FROM books
          WHERE title LIKE ?1 ESCAPE '\\'
             OR author LIKE ?1 ESCAPE '\\'
             OR series LIKE ?1 ESCAPE '\\'
          ORDER BY last_read_at IS NULL, last_read_at DESC, added_at DESC",
        BOOK_COLS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([pattern], row_to_book)?;
    rows.collect()
}

pub fn touch_last_read(conn: &Connection, id: &str, ts_ms: i64) -> rusqlite::Result<()> {
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
        let opts = SimpleFileOptions::default();
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
        let opts = SimpleFileOptions::default();
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
        let opts = SimpleFileOptions::default();
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
        assert!(std::path::Path::new(&r1.book.file_path).exists());
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
        assert!(std::path::Path::new(&result.book.file_path).exists());

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
        assert_eq!(crate::migrations::current_version(&conn).unwrap(), 2);
        drop(conn);

        // 重开已有库：迁移幂等跳过，版本不变、数据仍在。
        let conn = open_library(&path).unwrap();
        assert_eq!(crate::migrations::current_version(&conn).unwrap(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn make_epub_with_cover_bytes(title: &str, cover: &[u8]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = SimpleFileOptions::default();
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
        w.write_all(b"<html><body><p>text</p></body></html>").unwrap();
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn import_generates_thumbnail_from_real_cover() {
        let dir = temp_library("thumb");
        let conn = open_library(&dir.join("library.sqlite")).unwrap();
        // 用 image 生成一张真实 300×450 PNG 作封面
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(300, 450, image::Rgb([120, 90, 160])))
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
