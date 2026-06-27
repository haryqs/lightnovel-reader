//! 持久化解析缓存（v0.4 性能件）：把已解析的 `BookInfo` 与清洗后的章节 HTML 落盘，
//! 二次开书 / 翻已读章直接读盘，跳过 OPF/NCX 解析与 HTML 清洗（首次开书仍解析一次）。
//!
//! key = bookId（EPUB 内容哈希前缀，见 [`crate::compute_book_id`]）。内容一变 id 就变、
//! 缓存目录随之不同，旧缓存自动失效——不存在脏读。`CACHE_VERSION` 随解析/清洗逻辑的
//! 破坏性变化递增，使旧版本缓存整体作废（清洗规则改了、老缓存别再用）。
//!
//! 全部操作 **fail-open**：任何缓存读/写/反序列化错误都不应让开书失败，最多损失提速。
//! 因此读接口返回 `Option`（None = 未命中或损坏，调用方重新解析），写接口吞掉错误。
//!
//! 只缓存正文 HTML（文本，体量小）；插图仍按需经 `reader-img://` 从 EPUB 读取，不进缓存。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::epub_parser::BookInfo;

/// 解析/清洗逻辑发生破坏性变化时 +1，使旧缓存整体作废。
const CACHE_VERSION: u32 = 1;

fn book_cache_dir(cache_root: &Path, book_id: &str) -> PathBuf {
    cache_root
        .join("parsed")
        .join(format!("v{}", CACHE_VERSION))
        .join(book_id)
}

/// 章节 href → 文件名安全的短哈希（避免 href 里的 `/`、`#`、查询串等污染路径）。
fn href_key(href: &str) -> String {
    let digest = Sha256::digest(href.as_bytes());
    digest
        .iter()
        .take(16)
        .map(|b| format!("{:02x}", b))
        .collect()
}

/// 读缓存的 `BookInfo`；未命中或损坏返回 None。
pub fn load_book_info(cache_root: &Path, book_id: &str) -> Option<BookInfo> {
    let path = book_cache_dir(cache_root, book_id).join("info.json");
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// 落盘 `BookInfo`；出错静默忽略（fail-open）。
pub fn store_book_info(cache_root: &Path, book_id: &str, info: &BookInfo) {
    if let Ok(json) = serde_json::to_vec(info) {
        let dir = book_cache_dir(cache_root, book_id);
        let _ = write_atomic(&dir, "info.json", &json);
    }
}

/// 读缓存的清洗后章节 HTML；未命中返回 None。
pub fn load_chapter(cache_root: &Path, book_id: &str, href: &str) -> Option<String> {
    let path = book_cache_dir(cache_root, book_id)
        .join("ch")
        .join(format!("{}.html", href_key(href)));
    std::fs::read_to_string(path).ok()
}

/// 落盘清洗后章节 HTML；出错静默忽略（fail-open）。
pub fn store_chapter(cache_root: &Path, book_id: &str, href: &str, html: &str) {
    let dir = book_cache_dir(cache_root, book_id).join("ch");
    let _ = write_atomic(&dir, &format!("{}.html", href_key(href)), html.as_bytes());
}

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// 原子写：先写临时文件再 rename，避免读到半截文件。临时名带进程 id + 自增序号防并发碰撞。
fn write_atomic(dir: &Path, file: &str, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = dir.join(format!(".{}.{}.{}.tmp", file, std::process::id(), seq));
    std::fs::write(&tmp, bytes)?;
    match std::fs::rename(&tmp, dir.join(file)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::epub_parser::{EpubMetadata, SpineItem, TocItem};

    fn tmp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "reading-core-cachetest-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_info() -> BookInfo {
        BookInfo {
            metadata: EpubMetadata {
                title: "凉宫春日的忧郁".into(),
                author: Some("谷川流".into()),
                language: Some("zh".into()),
                description: None,
                series: Some("凉宫春日".into()),
                series_index: Some(1.0),
            },
            toc: vec![TocItem {
                label: "第一章".into(),
                href: "Text/ch1.xhtml".into(),
                subitems: vec![],
            }],
            spine: vec![SpineItem {
                id: "ch1".into(),
                href: "Text/ch1.xhtml".into(),
            }],
        }
    }

    #[test]
    fn book_info_roundtrip() {
        let root = tmp_root("info");
        let id = "abc123";
        assert!(load_book_info(&root, id).is_none(), "未写入前应未命中");

        store_book_info(&root, id, &sample_info());
        let got = load_book_info(&root, id).expect("应命中");
        assert_eq!(got.metadata.title, "凉宫春日的忧郁");
        assert_eq!(got.metadata.series_index, Some(1.0));
        assert_eq!(got.spine.len(), 1);
        assert_eq!(got.toc[0].label, "第一章");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn chapter_roundtrip_and_isolation_by_book() {
        let root = tmp_root("chapter");
        store_chapter(&root, "book-a", "Text/ch1.xhtml", "<p>甲书第一章</p>");
        store_chapter(&root, "book-b", "Text/ch1.xhtml", "<p>乙书第一章</p>");

        // 同 href 不同 bookId 互不串味
        assert_eq!(
            load_chapter(&root, "book-a", "Text/ch1.xhtml").as_deref(),
            Some("<p>甲书第一章</p>")
        );
        assert_eq!(
            load_chapter(&root, "book-b", "Text/ch1.xhtml").as_deref(),
            Some("<p>乙书第一章</p>")
        );
        // 不同 href 未命中
        assert!(load_chapter(&root, "book-a", "Text/ch2.xhtml").is_none());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn corrupt_info_fails_open() {
        let root = tmp_root("corrupt");
        let id = "deadbeef";
        // 手动写入坏 JSON，模拟半截/损坏缓存
        let dir = book_cache_dir(&root, id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("info.json"), b"{ not valid json").unwrap();

        // fail-open：返回 None 而非 panic，调用方据此重新解析
        assert!(load_book_info(&root, id).is_none());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn href_key_is_stable_and_distinct() {
        assert_eq!(href_key("Text/ch1.xhtml"), href_key("Text/ch1.xhtml"));
        assert_ne!(href_key("Text/ch1.xhtml"), href_key("Text/ch2.xhtml"));
        // 文件名安全：仅十六进制
        assert!(href_key("a/b#c?d=1").chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn cache_path_carries_version() {
        let dir = book_cache_dir(Path::new("/root"), "bookid");
        assert!(dir.to_string_lossy().contains("parsed"));
        assert!(dir
            .to_string_lossy()
            .contains(&format!("v{}", CACHE_VERSION)));
    }
}
