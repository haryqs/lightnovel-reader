//! reading-core：阅读器跨端核心（方案文档 7 的“胖核”）。
//!
//! 本 crate 不依赖任何 UI / 平台框架。所有平台壳（Tauri 桌面、将来的
//! Android/iOS/鸿蒙壳）通过各自的胶水层调用这里的能力；胶水层只做
//! 消息搬运，业务逻辑一律写在本 crate 内。

pub mod connectors;
pub mod epub_parser;
pub mod html_sanitizer;
pub mod library;
pub mod migrations;
pub mod parse_cache;
pub mod plugin_host;
pub mod plugin_manifest;
pub mod plugin_package;
pub mod plugin_store;
pub mod storage;

// 壳需要与 core 共用同一个 rusqlite（类型必须同源），统一从这里取。
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
