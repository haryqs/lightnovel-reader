//! 标注持久化（SQLite via rusqlite）。
//!
//! 设计：locator 整体以 JSON 存入 `locator` 列——锚模型将来加字段不必 ALTER TABLE。
//! `chapter_href` 从 locator 里抽一份单列出来，便于按章分组（导出）与查询。
//! `book_id` 由前端用 EPUB 内容哈希算出 → 标注跟着书走，文件改名/移动也不丢。

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub id: String,
    pub book_id: String,
    /// highlight | note | bookmark。wire 字段名为 `kind`（与前端协议一致），仅数据库列名是 `type`。
    pub kind: String,
    pub color: Option<String>,
    pub locator: serde_json::Value, // 与前端 Locator 同形，整体存 JSON
    pub note: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS annotations (
  id           TEXT PRIMARY KEY,
  book_id      TEXT NOT NULL,
  type         TEXT NOT NULL,          -- highlight | note | bookmark
  color        TEXT,
  chapter_href TEXT NOT NULL,
  locator      TEXT NOT NULL,          -- JSON 序列化的 Locator（含 anchor）
  note         TEXT,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_annotations_book ON annotations(book_id);

CREATE TABLE IF NOT EXISTS reading_state (
  book_id          TEXT PRIMARY KEY,   -- 与标注同源：EPUB 内容哈希
  chapter_href     TEXT NOT NULL,      -- spine 规范形式的章节 href
  chapter_progress REAL NOT NULL,      -- 章内进度 0..1（页索引比例）
  percentage       REAL NOT NULL,      -- 全书进度 0..1（展示用）
  updated_at       INTEGER NOT NULL
);
"#;

pub fn init(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

pub fn save(conn: &Connection, a: &Annotation) -> rusqlite::Result<()> {
    let chapter_href = a
        .locator
        .get("chapterHref")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let locator_json = a.locator.to_string();
    conn.execute(
        "INSERT OR REPLACE INTO annotations
           (id, book_id, type, color, chapter_href, locator, note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            a.id, a.book_id, a.kind, a.color, chapter_href, locator_json, a.note,
            a.created_at, a.updated_at
        ],
    )?;
    Ok(())
}

pub fn list(conn: &Connection, book_id: &str) -> rusqlite::Result<Vec<Annotation>> {
    let mut stmt = conn.prepare(
        "SELECT id, book_id, type, color, locator, note, created_at, updated_at
           FROM annotations WHERE book_id = ?1 ORDER BY created_at",
    )?;
    let rows = stmt.query_map([book_id], |row| {
        let locator_str: String = row.get(4)?;
        let locator =
            serde_json::from_str(&locator_str).unwrap_or(serde_json::Value::Null);
        Ok(Annotation {
            id: row.get(0)?,
            book_id: row.get(1)?,
            kind: row.get(2)?,
            color: row.get(3)?,
            locator,
            note: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?;
    rows.collect()
}

pub fn delete(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM annotations WHERE id = ?1", params![id])?;
    Ok(())
}

// —— 阅读进度 ——

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingProgress {
    pub book_id: String,
    pub chapter_href: String,
    pub chapter_progress: f64,
    pub percentage: f64,
    pub updated_at: i64,
}

pub fn save_progress(conn: &Connection, p: &ReadingProgress) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO reading_state
           (book_id, chapter_href, chapter_progress, percentage, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![p.book_id, p.chapter_href, p.chapter_progress, p.percentage, p.updated_at],
    )?;
    Ok(())
}

pub fn get_progress(conn: &Connection, book_id: &str) -> rusqlite::Result<Option<ReadingProgress>> {
    let mut stmt = conn.prepare(
        "SELECT book_id, chapter_href, chapter_progress, percentage, updated_at
           FROM reading_state WHERE book_id = ?1",
    )?;
    let mut rows = stmt.query_map([book_id], |row| {
        Ok(ReadingProgress {
            book_id: row.get(0)?,
            chapter_href: row.get(1)?,
            chapter_progress: row.get(2)?,
            percentage: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })?;
    rows.next().transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ann(id: &str, book: &str, href: &str, start: i64) -> Annotation {
        Annotation {
            id: id.into(),
            book_id: book.into(),
            kind: "highlight".into(),
            color: Some("yellow".into()),
            locator: json!({
                "bookId": book, "chapterHref": href,
                "anchor": { "start": start, "end": start + 4, "exact": "测试文本", "prefix": "", "suffix": "" }
            }),
            note: None,
            created_at: 1000 + start,
            updated_at: 1000 + start,
        }
    }

    #[test]
    fn progress_roundtrip_and_upsert() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        assert!(get_progress(&conn, "book1").unwrap().is_none());

        let p = ReadingProgress {
            book_id: "book1".into(),
            chapter_href: "text/ch3.xhtml".into(),
            chapter_progress: 0.5,
            percentage: 0.21,
            updated_at: 1000,
        };
        save_progress(&conn, &p).unwrap();
        let got = get_progress(&conn, "book1").unwrap().unwrap();
        assert_eq!(got.chapter_href, "text/ch3.xhtml");
        assert!((got.chapter_progress - 0.5).abs() < 1e-9);

        // 同书覆盖而非新增
        let p2 = ReadingProgress { chapter_progress: 0.75, updated_at: 2000, ..p };
        save_progress(&conn, &p2).unwrap();
        let got = get_progress(&conn, "book1").unwrap().unwrap();
        assert!((got.chapter_progress - 0.75).abs() < 1e-9);
        assert_eq!(got.updated_at, 2000);
    }

    #[test]
    fn save_list_delete_roundtrip() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        save(&conn, &ann("a1", "book1", "ch1.html", 10)).unwrap();
        save(&conn, &ann("a2", "book1", "ch2.html", 20)).unwrap();
        save(&conn, &ann("b1", "book2", "ch1.html", 5)).unwrap();

        // 按 book_id 过滤 + 按 created_at 排序
        let l1 = list(&conn, "book1").unwrap();
        assert_eq!(l1.len(), 2);
        assert_eq!(l1[0].id, "a1");
        assert_eq!(l1[1].id, "a2");
        // locator JSON 往返完好（含中文）
        assert_eq!(l1[0].locator["chapterHref"], "ch1.html");
        assert_eq!(l1[0].locator["anchor"]["exact"], "测试文本");
        assert_eq!(list(&conn, "book2").unwrap().len(), 1);

        // INSERT OR REPLACE：同 id 覆盖而非新增
        let mut updated = ann("a1", "book1", "ch1.html", 10);
        updated.note = Some("改了笔记".into());
        save(&conn, &updated).unwrap();
        let l1b = list(&conn, "book1").unwrap();
        assert_eq!(l1b.len(), 2);
        assert_eq!(l1b[0].note.as_deref(), Some("改了笔记"));

        // 删除
        delete(&conn, "a1").unwrap();
        let l1c = list(&conn, "book1").unwrap();
        assert_eq!(l1c.len(), 1);
        assert_eq!(l1c[0].id, "a2");
    }
}
