//! reading-core：阅读器跨端核心（方案文档 7 的“胖核”）。
//!
//! 本 crate 不依赖任何 UI / 平台框架。所有平台壳（Tauri 桌面、将来的
//! Android/iOS/鸿蒙壳）通过各自的胶水层调用这里的能力；胶水层只做
//! 消息搬运，业务逻辑一律写在本 crate 内。

#[cfg(feature = "native")]
pub mod backup;
#[cfg(feature = "native")]
pub mod connectors;
pub mod epub_parser;
pub mod html_sanitizer;
#[cfg(feature = "native")]
pub mod library;
#[cfg(feature = "native")]
pub mod migrations;
pub mod pagination;
pub mod parse_cache;
#[cfg(feature = "native")]
pub mod plugin_host;
#[cfg(feature = "native")]
pub mod plugin_manifest;
#[cfg(feature = "native")]
pub mod plugin_package;
#[cfg(feature = "native")]
pub mod plugin_repository;
#[cfg(feature = "native")]
pub mod plugin_runtime;
#[cfg(feature = "native")]
pub mod plugin_source;
#[cfg(feature = "native")]
pub mod plugin_store;
#[cfg(feature = "native")]
pub mod storage;
pub mod sync;

// 壳需要与 core 共用同一个 rusqlite（类型必须同源），统一从这里取。
#[cfg(feature = "native")]
pub use rusqlite;

use sha2::{Digest, Sha256};

/// bookId = SHA-256(epub 字节) 的前 32 hex，须与前端 computeBookId 完全一致。
pub fn compute_book_id(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
    hex[..32].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_id_is_stable_32_hex() {
        let id = compute_book_id(b"hello epub");
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(id, compute_book_id(b"hello epub"));
        assert_ne!(id, compute_book_id(b"hello epub!"));
    }
}

// ---- WASM-only exports (cfg wasm) ----

#[cfg(feature = "wasm")]
mod wasm_exports {
    use wasm_bindgen::prelude::*;

    /// 解析 EPUB 元数据，返回 JSON {metadata, toc, spine}
    #[wasm_bindgen]
    pub fn parse_epub_metadata(data: &[u8]) -> String {
        match crate::epub_parser::parse_book_info(data) {
            Ok(info) => serde_json::to_string(&info).unwrap_or_default(),
            Err(e) => {
                let escaped = e.replace('\\', "\\\\").replace('"', "\\\"");
                format!("{{\"error\":\"{}\"}}", escaped)
            }
        }
    }

    /// 提取并清洗章节 HTML（先解析元数据找到 spine，再读取对应文件）
    #[wasm_bindgen]
    pub fn get_chapter_html(data: &[u8], href: &str) -> String {
        let info = match crate::epub_parser::parse_book_info(data) {
            Ok(i) => i,
            Err(e) => return format!("<p>解析失败: {}</p>", e),
        };
        match crate::epub_parser::parse_single_chapter(data, href, &info) {
            Ok(html) => html,
            Err(e) => format!("<p>章节读取失败: {}</p>", e),
        }
    }
}
