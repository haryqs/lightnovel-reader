//! 已安装插件与正式书库来源之间的映射。
//!
//! 插件搜索本身不落库；只有用户显式收藏一本书时，才把经运行时校验的
//! `PluginBookDetail` 转为远程 `edition + source_record`。正文获取仍由来源调用处理，
//! 不会因为收藏动作自动下载或缓存。

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::compute_book_id;
use crate::connectors::{self, RemoteEntry};
use crate::library::{self, LibraryBook};
use crate::plugin_host::{ensure_method_allowed, PluginBookDetail, SourcePluginMethod};
use crate::plugin_manifest::{
    is_url_allowed_by_manifest, PluginCapability, PluginLegal, PluginLegalKind,
};
use crate::plugin_store::InstalledPlugin;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSourceDescriptor {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    pub legal: PluginLegal,
    pub capabilities: Vec<PluginCapability>,
}

pub fn list_enabled_sources(installed: &[InstalledPlugin]) -> Vec<PluginSourceDescriptor> {
    let mut sources = installed
        .iter()
        .filter(|plugin| plugin.enabled)
        .map(|plugin| PluginSourceDescriptor {
            id: plugin.manifest.id.clone(),
            name: plugin.manifest.name.clone(),
            description: plugin.manifest.description.clone(),
            language: plugin.manifest.language.clone(),
            legal: plugin.manifest.legal.clone(),
            capabilities: plugin.manifest.capabilities.clone(),
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });
    sources
}

pub fn collect_book(
    conn: &Connection,
    plugin: &InstalledPlugin,
    book: &PluginBookDetail,
    now_ms: i64,
) -> Result<LibraryBook, String> {
    ensure_method_allowed(plugin, SourcePluginMethod::GetBook)?;
    if book.title.trim().is_empty() {
        return Err("插件书籍标题不能为空".into());
    }
    if !is_url_allowed_by_manifest(&plugin.manifest, &book.url) {
        return Err("插件书籍 URL 不在 manifest 域名白名单内".into());
    }
    if book
        .cover_url
        .as_deref()
        .is_some_and(|url| !is_url_allowed_by_manifest(&plugin.manifest, url))
    {
        return Err("插件封面 URL 不在 manifest 域名白名单内".into());
    }

    let source_id = format!("plugin:{}", plugin.manifest.id);
    connectors::ensure_source(
        conn,
        &source_id,
        &plugin.manifest.name,
        "plugin",
        None,
        now_ms,
    )
    .map_err(|e| e.to_string())?;

    let remote_id = compute_book_id(format!("{}\0{}", plugin.manifest.id, book.url).as_bytes());
    let entry = RemoteEntry {
        remote_id,
        title: book.title.trim().to_string(),
        author: book.author.clone(),
        description: book.description.clone(),
        cover_url: book.cover_url.clone(),
        language: plugin.manifest.language.clone(),
        site_url: Some(book.url.clone()),
        acquisition_url: None,
        rights_status: rights_status(plugin.manifest.legal.kind).to_string(),
    };
    let edition_id = connectors::ingest(conn, &source_id, &[entry], now_ms)
        .map_err(|e| e.to_string())?
        .into_iter()
        .next()
        .ok_or_else(|| "插件来源收藏后没有生成书库条目".to_string())?;
    library::get_book(conn, &edition_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "插件来源收藏后无法读取书库条目".to_string())
}

fn rights_status(kind: PluginLegalKind) -> &'static str {
    match kind {
        PluginLegalKind::PublicDomain => "public_domain",
        PluginLegalKind::OpenLicense => "open_license",
        PluginLegalKind::OfficialFree => "official_free",
        PluginLegalKind::UserDeclared => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_manifest::{
        ManifestValidation, PluginLegal, PluginManifest, PluginPermission,
    };
    use std::path::Path;

    fn installed(id: &str, name: &str, legal: PluginLegalKind, enabled: bool) -> InstalledPlugin {
        InstalledPlugin {
            manifest: PluginManifest {
                api_version: "0.1".into(),
                id: id.into(),
                name: name.into(),
                version: "0.1.0".into(),
                description: Some("fixture".into()),
                author: None,
                language: Some("en".into()),
                entry: "plugin.js".into(),
                domains: vec!["example.com".into()],
                permissions: vec![PluginPermission::Http],
                capabilities: vec![],
                legal: PluginLegal {
                    kind: legal,
                    note: None,
                    terms_url: None,
                },
            },
            validation: ManifestValidation {
                official_repository_eligible: true,
                requires_user_legal_confirmation: false,
                requires_source_terms_confirmation: false,
                warnings: vec![],
            },
            entry_size: 10,
            installed_at: 1,
            enabled,
        }
    }

    fn detail(title: &str) -> PluginBookDetail {
        PluginBookDetail {
            url: "https://example.com/books/one".into(),
            title: title.into(),
            author: Some("Author".into()),
            cover_url: Some("https://example.com/covers/one.jpg".into()),
            description: Some("Description".into()),
            chapters: vec![],
        }
    }

    #[test]
    fn source_list_only_contains_enabled_plugins() {
        let sources = list_enabled_sources(&[
            installed("z-source", "Zulu", PluginLegalKind::OfficialFree, true),
            installed("a-source", "Alpha", PluginLegalKind::PublicDomain, true),
            installed(
                "off-source",
                "Disabled",
                PluginLegalKind::OpenLicense,
                false,
            ),
        ]);
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].id, "a-source");
        assert_eq!(sources[1].id, "z-source");
    }

    #[test]
    fn collecting_plugin_book_creates_idempotent_remote_source_record() {
        let conn = library::open_library(Path::new(":memory:")).unwrap();
        let plugin = installed(
            "fixture-source",
            "Fixture Source",
            PluginLegalKind::PublicDomain,
            true,
        );

        let first = collect_book(&conn, &plugin, &detail("First title"), 100).unwrap();
        assert_eq!(first.availability.as_deref(), Some("remote"));
        assert_eq!(first.rights_status.as_deref(), Some("public_domain"));
        assert_eq!(
            first.remote_url.as_deref(),
            Some("https://example.com/books/one")
        );

        let second = collect_book(&conn, &plugin, &detail("Updated title"), 200).unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(second.title, "Updated title");

        let records = library::list_source_records(&conn, &second.id).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source_name, "Fixture Source");
        assert_eq!(records[0].source_kind, "plugin");
        assert_eq!(records[0].last_checked_at, Some(200));
    }

    #[test]
    fn user_declared_collection_stays_unknown_and_disabled_is_rejected() {
        let conn = library::open_library(Path::new(":memory:")).unwrap();
        let user_plugin = installed(
            "user-source",
            "User Source",
            PluginLegalKind::UserDeclared,
            true,
        );
        let collected = collect_book(&conn, &user_plugin, &detail("User book"), 100).unwrap();
        assert_eq!(collected.rights_status.as_deref(), Some("unknown"));

        let disabled = installed(
            "disabled-source",
            "Disabled",
            PluginLegalKind::PublicDomain,
            false,
        );
        assert!(collect_book(&conn, &disabled, &detail("No"), 100)
            .unwrap_err()
            .contains("disabled"));
    }

    #[test]
    fn collection_rejects_urls_outside_manifest_domains() {
        let conn = library::open_library(Path::new(":memory:")).unwrap();
        let plugin = installed(
            "fixture-source",
            "Fixture Source",
            PluginLegalKind::PublicDomain,
            true,
        );
        let mut book = detail("Unsafe");
        book.url = "https://evil.example/books/one".into();
        assert!(collect_book(&conn, &plugin, &book, 100)
            .unwrap_err()
            .contains("白名单"));
    }
}
