//! Source plugin host API DTOs and policy gates.
//!
//! This module intentionally does not execute JavaScript. It defines the narrow
//! Rust-side contract that a future QuickJS/JavaScriptCore host must pass
//! through before making network, KV, or acquisition side effects.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::plugin_manifest::{
    is_url_allowed_by_manifest, PluginCapability, PluginLegalKind, PluginManifest, PluginPermission,
};
use crate::plugin_store::InstalledPlugin;

pub const DEFAULT_HTTP_TIMEOUT_MS: u64 = 15_000;
pub const MAX_HTTP_TIMEOUT_MS: u64 = 60_000;
pub const MAX_PLUGIN_HTTP_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PLUGIN_HTML_INPUT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PLUGIN_HTML_SELECTOR_LEN: usize = 1024;
pub const MAX_PLUGIN_LOG_MESSAGE_BYTES: usize = 4 * 1024;
pub const MAX_PLUGIN_RESULT_JSON_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PLUGIN_HEADERS: usize = 32;
pub const MAX_PLUGIN_HEADER_NAME_LEN: usize = 64;
pub const MAX_PLUGIN_HEADER_VALUE_LEN: usize = 1024;
pub const MAX_KV_KEY_LEN: usize = 128;
pub const MAX_KV_VALUE_LEN: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourcePluginMethod {
    Search,
    GetBook,
    GetChapter,
    Browse,
    ResolveUrl,
    FetchMetadata,
    Acquire,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSearchRequest {
    pub query: String,
    pub page: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSearchPage {
    pub results: Vec<PluginSearchResult>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSearchResult {
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub cover_url: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginBookDetail {
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub cover_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub chapters: Vec<PluginChapterRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginChapterRef {
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginChapterContent {
    pub title: String,
    pub html: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostHttpGetRequest {
    pub url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostHttpGetPlan {
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub timeout_ms: u64,
    pub ignored_headers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostHttpResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AcquireMode {
    MetadataOnly,
    Download,
    CacheForReading,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginRightsStatus {
    PublicDomain,
    OpenLicense,
    OfficialFree,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcquireProposal {
    pub url: String,
    pub rights_status: PluginRightsStatus,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcquireHostDecision {
    pub url: String,
    pub rights_status: PluginRightsStatus,
    pub mode: AcquireMode,
    pub may_download: bool,
    pub may_cache_for_reading: bool,
}

pub fn ensure_method_allowed(
    plugin: &InstalledPlugin,
    method: SourcePluginMethod,
) -> Result<(), String> {
    if !plugin.enabled {
        return Err("plugin is disabled".into());
    }
    if let Some(capability) = required_capability(method) {
        if !plugin.manifest.capabilities.contains(&capability) {
            return Err(format!(
                "plugin capability {:?} is required for {:?}",
                capability, method
            ));
        }
    }
    Ok(())
}

pub fn plan_http_get(
    manifest: &PluginManifest,
    request: HostHttpGetRequest,
) -> Result<HostHttpGetPlan, String> {
    if !manifest.permissions.contains(&PluginPermission::Http) {
        return Err("plugin does not have http permission".into());
    }
    if !is_url_allowed_by_manifest(manifest, &request.url) {
        return Err("plugin http url is outside manifest domains".into());
    }
    if request.headers.len() > MAX_PLUGIN_HEADERS {
        return Err(format!(
            "plugin http headers exceed limit {MAX_PLUGIN_HEADERS}"
        ));
    }
    let timeout_ms = request.timeout_ms.unwrap_or(DEFAULT_HTTP_TIMEOUT_MS);
    if timeout_ms == 0 || timeout_ms > MAX_HTTP_TIMEOUT_MS {
        return Err(format!(
            "plugin http timeout must be 1..={MAX_HTTP_TIMEOUT_MS} ms"
        ));
    }

    let (headers, ignored_headers) = sanitize_plugin_headers(request.headers)?;
    Ok(HostHttpGetPlan {
        url: request.url,
        headers,
        timeout_ms,
        ignored_headers,
    })
}

pub fn ensure_kv_access(
    manifest: &PluginManifest,
    key: &str,
    value: Option<&str>,
) -> Result<(), String> {
    if !manifest.permissions.contains(&PluginPermission::Kv) {
        return Err("plugin does not have kv permission".into());
    }
    if key.is_empty() || key.chars().count() > MAX_KV_KEY_LEN || key.chars().any(char::is_control) {
        return Err(format!(
            "plugin kv key must be 1..={MAX_KV_KEY_LEN} non-control chars"
        ));
    }
    if let Some(value) = value {
        if value.len() > MAX_KV_VALUE_LEN {
            return Err(format!(
                "plugin kv value exceeds limit {MAX_KV_VALUE_LEN} bytes"
            ));
        }
    }
    Ok(())
}

pub fn authorize_acquire_proposal(
    manifest: &PluginManifest,
    mode: AcquireMode,
    proposal: AcquireProposal,
) -> Result<AcquireHostDecision, String> {
    if !manifest.capabilities.contains(&PluginCapability::Acquire) {
        return Err("plugin does not declare acquire capability".into());
    }
    if !is_url_allowed_by_manifest(manifest, &proposal.url) {
        return Err("plugin acquire url is outside manifest domains".into());
    }

    if mode == AcquireMode::MetadataOnly {
        return Ok(AcquireHostDecision {
            url: proposal.url,
            rights_status: proposal.rights_status,
            mode,
            may_download: false,
            may_cache_for_reading: false,
        });
    }

    let lawful_open_resource = matches!(
        (manifest.legal.kind, proposal.rights_status),
        (
            PluginLegalKind::PublicDomain,
            PluginRightsStatus::PublicDomain
        ) | (
            PluginLegalKind::OpenLicense,
            PluginRightsStatus::OpenLicense
        ) | (
            PluginLegalKind::OpenLicense,
            PluginRightsStatus::PublicDomain
        )
    );
    if !lawful_open_resource {
        return Err(
            "plugin acquire download/cache is limited to public-domain or open-license resources"
                .into(),
        );
    }

    Ok(AcquireHostDecision {
        url: proposal.url,
        rights_status: proposal.rights_status,
        mode,
        may_download: true,
        may_cache_for_reading: mode == AcquireMode::CacheForReading,
    })
}

fn required_capability(method: SourcePluginMethod) -> Option<PluginCapability> {
    match method {
        SourcePluginMethod::Search
        | SourcePluginMethod::GetBook
        | SourcePluginMethod::GetChapter => None,
        SourcePluginMethod::Browse => Some(PluginCapability::Browse),
        SourcePluginMethod::ResolveUrl => Some(PluginCapability::ResolveUrl),
        SourcePluginMethod::FetchMetadata => Some(PluginCapability::FetchMetadata),
        SourcePluginMethod::Acquire => Some(PluginCapability::Acquire),
    }
}

fn sanitize_plugin_headers(
    headers: BTreeMap<String, String>,
) -> Result<(BTreeMap<String, String>, Vec<String>), String> {
    let mut allowed = BTreeMap::new();
    let mut ignored = Vec::new();
    for (name, value) in headers {
        let normalized = name.trim().to_ascii_lowercase();
        if !is_valid_header_name(&normalized) {
            return Err(format!("invalid plugin http header name: {name}"));
        }
        if value.len() > MAX_PLUGIN_HEADER_VALUE_LEN || value.contains(['\r', '\n']) {
            return Err(format!("invalid plugin http header value for {name}"));
        }
        if is_reserved_header(&normalized) {
            ignored.push(normalized);
            continue;
        }
        allowed.insert(normalized, value);
    }
    Ok((allowed, ignored))
}

fn is_valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_PLUGIN_HEADER_NAME_LEN
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn is_reserved_header(name: &str) -> bool {
    matches!(
        name,
        "authorization" | "cookie" | "host" | "origin" | "referer" | "user-agent"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_manifest::{ManifestValidation, PluginLegal};

    fn sample_manifest(
        legal_kind: PluginLegalKind,
        permissions: Vec<PluginPermission>,
        capabilities: Vec<PluginCapability>,
    ) -> PluginManifest {
        PluginManifest {
            api_version: "0.1".into(),
            id: "sample-source".into(),
            name: "Sample Source".into(),
            version: "0.1.0".into(),
            description: None,
            author: None,
            language: Some("ja".into()),
            entry: "plugin.js".into(),
            domains: vec!["example.org".into()],
            permissions,
            capabilities,
            legal: PluginLegal {
                kind: legal_kind,
                note: None,
                terms_url: None,
            },
        }
    }

    fn installed(manifest: PluginManifest, enabled: bool) -> InstalledPlugin {
        InstalledPlugin {
            manifest,
            validation: ManifestValidation {
                official_repository_eligible: true,
                requires_user_legal_confirmation: false,
                requires_source_terms_confirmation: false,
                warnings: Vec::new(),
            },
            entry_size: 1,
            installed_at: 42,
            enabled,
        }
    }

    #[test]
    fn required_methods_do_not_need_optional_capability() {
        let plugin = installed(
            sample_manifest(PluginLegalKind::PublicDomain, Vec::new(), Vec::new()),
            true,
        );
        ensure_method_allowed(&plugin, SourcePluginMethod::Search).unwrap();
        ensure_method_allowed(&plugin, SourcePluginMethod::GetBook).unwrap();
        ensure_method_allowed(&plugin, SourcePluginMethod::GetChapter).unwrap();
    }

    #[test]
    fn optional_methods_require_declared_capability() {
        let plugin = installed(
            sample_manifest(PluginLegalKind::PublicDomain, Vec::new(), Vec::new()),
            true,
        );
        assert!(ensure_method_allowed(&plugin, SourcePluginMethod::Browse)
            .unwrap_err()
            .contains("capability"));

        let plugin = installed(
            sample_manifest(
                PluginLegalKind::PublicDomain,
                Vec::new(),
                vec![PluginCapability::Browse],
            ),
            true,
        );
        ensure_method_allowed(&plugin, SourcePluginMethod::Browse).unwrap();
    }

    #[test]
    fn disabled_plugin_cannot_run_any_method() {
        let plugin = installed(
            sample_manifest(PluginLegalKind::PublicDomain, Vec::new(), Vec::new()),
            false,
        );
        assert!(ensure_method_allowed(&plugin, SourcePluginMethod::Search)
            .unwrap_err()
            .contains("disabled"));
    }

    #[test]
    fn http_get_requires_permission_and_exact_domain() {
        let manifest = sample_manifest(PluginLegalKind::PublicDomain, Vec::new(), Vec::new());
        let err = plan_http_get(
            &manifest,
            HostHttpGetRequest {
                url: "https://example.org/book".into(),
                headers: BTreeMap::new(),
                timeout_ms: None,
            },
        )
        .unwrap_err();
        assert!(err.contains("http permission"));

        let manifest = sample_manifest(
            PluginLegalKind::PublicDomain,
            vec![PluginPermission::Http],
            Vec::new(),
        );
        assert!(plan_http_get(
            &manifest,
            HostHttpGetRequest {
                url: "https://evil.example.org/book".into(),
                headers: BTreeMap::new(),
                timeout_ms: None,
            },
        )
        .unwrap_err()
        .contains("outside"));

        let plan = plan_http_get(
            &manifest,
            HostHttpGetRequest {
                url: "https://example.org/book".into(),
                headers: BTreeMap::new(),
                timeout_ms: None,
            },
        )
        .unwrap();
        assert_eq!(plan.timeout_ms, DEFAULT_HTTP_TIMEOUT_MS);
    }

    #[test]
    fn http_get_sanitizes_headers_and_rejects_timeout_over_limit() {
        let manifest = sample_manifest(
            PluginLegalKind::PublicDomain,
            vec![PluginPermission::Http],
            Vec::new(),
        );
        let mut headers = BTreeMap::new();
        headers.insert("User-Agent".into(), "fake".into());
        headers.insert("X-Trace".into(), "ok".into());
        let plan = plan_http_get(
            &manifest,
            HostHttpGetRequest {
                url: "https://example.org/book".into(),
                headers,
                timeout_ms: Some(60_000),
            },
        )
        .unwrap();
        assert_eq!(plan.headers.get("x-trace").map(String::as_str), Some("ok"));
        assert_eq!(plan.ignored_headers, vec!["user-agent"]);

        assert!(plan_http_get(
            &manifest,
            HostHttpGetRequest {
                url: "https://example.org/book".into(),
                headers: BTreeMap::new(),
                timeout_ms: Some(60_001),
            },
        )
        .unwrap_err()
        .contains("timeout"));
    }

    #[test]
    fn kv_access_requires_permission_and_size_limits() {
        let manifest = sample_manifest(PluginLegalKind::PublicDomain, Vec::new(), Vec::new());
        assert!(ensure_kv_access(&manifest, "token", None)
            .unwrap_err()
            .contains("kv permission"));

        let manifest = sample_manifest(
            PluginLegalKind::PublicDomain,
            vec![PluginPermission::Kv],
            Vec::new(),
        );
        ensure_kv_access(&manifest, "token", Some("value")).unwrap();
        assert!(ensure_kv_access(&manifest, "", None).is_err());
        assert!(
            ensure_kv_access(&manifest, "token", Some(&"x".repeat(MAX_KV_VALUE_LEN + 1))).is_err()
        );
    }

    #[test]
    fn acquire_metadata_only_does_not_download() {
        let manifest = sample_manifest(
            PluginLegalKind::OfficialFree,
            Vec::new(),
            vec![PluginCapability::Acquire],
        );
        let decision = authorize_acquire_proposal(
            &manifest,
            AcquireMode::MetadataOnly,
            AcquireProposal {
                url: "https://example.org/book".into(),
                rights_status: PluginRightsStatus::OfficialFree,
                mime_type: None,
                note: None,
            },
        )
        .unwrap();
        assert!(!decision.may_download);
        assert!(!decision.may_cache_for_reading);
    }

    #[test]
    fn acquire_download_only_allows_open_resources() {
        let manifest = sample_manifest(
            PluginLegalKind::OpenLicense,
            Vec::new(),
            vec![PluginCapability::Acquire],
        );
        let decision = authorize_acquire_proposal(
            &manifest,
            AcquireMode::CacheForReading,
            AcquireProposal {
                url: "https://example.org/book.epub".into(),
                rights_status: PluginRightsStatus::OpenLicense,
                mime_type: Some("application/epub+zip".into()),
                note: None,
            },
        )
        .unwrap();
        assert!(decision.may_download);
        assert!(decision.may_cache_for_reading);

        let manifest = sample_manifest(
            PluginLegalKind::OfficialFree,
            Vec::new(),
            vec![PluginCapability::Acquire],
        );
        assert!(authorize_acquire_proposal(
            &manifest,
            AcquireMode::Download,
            AcquireProposal {
                url: "https://example.org/book".into(),
                rights_status: PluginRightsStatus::OfficialFree,
                mime_type: None,
                note: None,
            },
        )
        .unwrap_err()
        .contains("public-domain or open-license"));
    }

    #[test]
    fn acquire_requires_capability_and_domain() {
        let manifest = sample_manifest(PluginLegalKind::PublicDomain, Vec::new(), Vec::new());
        assert!(authorize_acquire_proposal(
            &manifest,
            AcquireMode::Download,
            AcquireProposal {
                url: "https://example.org/book".into(),
                rights_status: PluginRightsStatus::PublicDomain,
                mime_type: None,
                note: None,
            },
        )
        .unwrap_err()
        .contains("capability"));

        let manifest = sample_manifest(
            PluginLegalKind::PublicDomain,
            Vec::new(),
            vec![PluginCapability::Acquire],
        );
        assert!(authorize_acquire_proposal(
            &manifest,
            AcquireMode::Download,
            AcquireProposal {
                url: "https://other.example.org/book".into(),
                rights_status: PluginRightsStatus::PublicDomain,
                mime_type: None,
                note: None,
            },
        )
        .unwrap_err()
        .contains("outside"));
    }
}
