use serde::{Deserialize, Serialize};

pub const SUPPORTED_API_VERSION: &str = "0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub api_version: String,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    pub entry: String,
    pub domains: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,
    pub legal: PluginLegal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginPermission {
    Http,
    Kv,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginCapability {
    Browse,
    ResolveUrl,
    FetchMetadata,
    Acquire,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginLegal {
    pub kind: PluginLegalKind,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginLegalKind {
    PublicDomain,
    OpenLicense,
    OfficialFree,
    UserDeclared,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestValidation {
    pub official_repository_eligible: bool,
    pub requires_user_legal_confirmation: bool,
    pub warnings: Vec<String>,
}

pub fn parse_manifest_json(json: &str) -> Result<PluginManifest, String> {
    let manifest: PluginManifest =
        serde_json::from_str(json).map_err(|e| format!("invalid plugin manifest JSON: {e}"))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_manifest(manifest: &PluginManifest) -> Result<ManifestValidation, String> {
    if manifest.api_version != SUPPORTED_API_VERSION {
        return Err(format!(
            "unsupported plugin apiVersion {}, expected {}",
            manifest.api_version, SUPPORTED_API_VERSION
        ));
    }
    if !is_valid_plugin_id(&manifest.id) {
        return Err("plugin id must match ^[a-z0-9][a-z0-9-]{1,63}$".into());
    }
    if manifest.name.trim().is_empty() || manifest.name.chars().count() > 64 {
        return Err("plugin name must be 1..64 characters".into());
    }
    if !is_semver_triplet(&manifest.version) {
        return Err("plugin version must be x.y.z".into());
    }
    if !is_safe_entry_name(&manifest.entry) {
        return Err("plugin entry must be a single .js file name".into());
    }
    if manifest.domains.is_empty() {
        return Err("plugin manifest must declare at least one domain".into());
    }
    for domain in &manifest.domains {
        if !is_valid_domain(domain) {
            return Err(format!("invalid plugin domain: {domain}"));
        }
    }
    if has_duplicates(&manifest.domains) {
        return Err("plugin domains must be unique".into());
    }
    if has_duplicates(&manifest.permissions) {
        return Err("plugin permissions must be unique".into());
    }
    if has_duplicates(&manifest.capabilities) {
        return Err("plugin capabilities must be unique".into());
    }
    if manifest
        .description
        .as_deref()
        .is_some_and(|s| s.chars().count() > 500)
    {
        return Err("plugin description must be <= 500 characters".into());
    }
    if manifest
        .author
        .as_deref()
        .is_some_and(|s| s.chars().count() > 128)
    {
        return Err("plugin author must be <= 128 characters".into());
    }
    if manifest
        .legal
        .note
        .as_deref()
        .is_some_and(|s| s.chars().count() > 500)
    {
        return Err("plugin legal.note must be <= 500 characters".into());
    }

    let requires_user_legal_confirmation = manifest.legal.kind == PluginLegalKind::UserDeclared;
    let official_repository_eligible = !requires_user_legal_confirmation;
    let mut warnings = Vec::new();
    if manifest.capabilities.contains(&PluginCapability::Acquire)
        && manifest.legal.kind == PluginLegalKind::OfficialFree
    {
        warnings.push(
            "official-free acquire must be gated by source ToS and host rate limits".to_string(),
        );
    }
    if requires_user_legal_confirmation {
        warnings.push(
            "user-declared plugins require explicit install-time legal confirmation".to_string(),
        );
    }

    Ok(ManifestValidation {
        official_repository_eligible,
        requires_user_legal_confirmation,
        warnings,
    })
}

pub fn is_url_allowed_by_manifest(manifest: &PluginManifest, url: &str) -> bool {
    let Some(host) = http_host(url) else {
        return false;
    };
    manifest
        .domains
        .iter()
        .any(|domain| domain.eq_ignore_ascii_case(&host))
}

fn is_valid_plugin_id(id: &str) -> bool {
    let len = id.len();
    len >= 2
        && len <= 64
        && id
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn is_semver_triplet(version: &str) -> bool {
    let mut parts = version.split('.');
    let Some(major) = parts.next() else {
        return false;
    };
    let Some(minor) = parts.next() else {
        return false;
    };
    let Some(patch) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && [major, minor, patch]
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

fn is_safe_entry_name(entry: &str) -> bool {
    entry.ends_with(".js")
        && entry.len() > ".js".len()
        && !entry.contains('/')
        && !entry.contains('\\')
        && !entry.contains("..")
}

fn is_valid_domain(domain: &str) -> bool {
    if domain.is_empty()
        || domain.len() > 253
        || domain.contains("..")
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return false;
    }
    let labels: Vec<&str> = domain.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
        && labels
            .last()
            .is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_lowercase()))
}

fn has_duplicates<T: PartialEq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(idx, value)| values.iter().skip(idx + 1).any(|other| other == value))
}

fn http_host(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    let host = host
        .split_once(':')
        .map(|(host, _)| host)
        .unwrap_or(host)
        .to_ascii_lowercase();
    if is_valid_domain(&host) {
        Some(host)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest_json(extra: &str) -> String {
        format!(
            r#"{{
  "apiVersion": "0.1",
  "id": "aozora-bunko",
  "name": "Aozora Bunko",
  "version": "0.1.0",
  "entry": "plugin.js",
  "domains": ["www.aozora.gr.jp"],
  "permissions": ["http"],
  "legal": {{ "kind": "public-domain" }}
  {extra}
}}"#
        )
    }

    #[test]
    fn parses_valid_manifest_and_policy_flags() {
        let manifest = parse_manifest_json(&valid_manifest_json(
            r#",
  "capabilities": ["browse", "fetchMetadata"]"#,
        ))
        .unwrap();
        assert_eq!(manifest.id, "aozora-bunko");
        assert_eq!(manifest.permissions, vec![PluginPermission::Http]);
        assert_eq!(
            manifest.capabilities,
            vec![PluginCapability::Browse, PluginCapability::FetchMetadata]
        );

        let policy = validate_manifest(&manifest).unwrap();
        assert!(policy.official_repository_eligible);
        assert!(!policy.requires_user_legal_confirmation);
        assert!(policy.warnings.is_empty());
    }

    #[test]
    fn rejects_unsupported_api_version() {
        let json =
            valid_manifest_json("").replace("\"apiVersion\": \"0.1\"", "\"apiVersion\": \"9.9\"");
        let err = parse_manifest_json(&json).unwrap_err();
        assert!(err.contains("unsupported plugin apiVersion"));
    }

    #[test]
    fn rejects_path_traversal_entry() {
        let json = valid_manifest_json("")
            .replace("\"entry\": \"plugin.js\"", "\"entry\": \"../plugin.js\"");
        let err = parse_manifest_json(&json).unwrap_err();
        assert!(err.contains("entry"));
    }

    #[test]
    fn rejects_duplicate_domains_permissions_and_capabilities() {
        let dup_domain = valid_manifest_json("").replace(
            "\"domains\": [\"www.aozora.gr.jp\"]",
            "\"domains\": [\"www.aozora.gr.jp\", \"www.aozora.gr.jp\"]",
        );
        assert!(parse_manifest_json(&dup_domain)
            .unwrap_err()
            .contains("domains must be unique"));

        let dup_permission = valid_manifest_json("").replace(
            "\"permissions\": [\"http\"]",
            "\"permissions\": [\"http\", \"http\"]",
        );
        assert!(parse_manifest_json(&dup_permission)
            .unwrap_err()
            .contains("permissions must be unique"));

        let dup_capability = valid_manifest_json(
            r#",
  "capabilities": ["browse", "browse"]"#,
        );
        assert!(parse_manifest_json(&dup_capability)
            .unwrap_err()
            .contains("capabilities must be unique"));
    }

    #[test]
    fn user_declared_requires_explicit_confirmation_and_is_not_official_repo_eligible() {
        let json = valid_manifest_json("").replace(
            "\"kind\": \"public-domain\"",
            "\"kind\": \"user-declared\", \"note\": \"user takes responsibility\"",
        );
        let manifest = parse_manifest_json(&json).unwrap();
        let policy = validate_manifest(&manifest).unwrap();
        assert!(!policy.official_repository_eligible);
        assert!(policy.requires_user_legal_confirmation);
        assert_eq!(policy.warnings.len(), 1);
    }

    #[test]
    fn official_free_acquire_warns_for_tos_gate() {
        let json = valid_manifest_json(
            r#",
  "capabilities": ["acquire"]"#,
        )
        .replace("\"kind\": \"public-domain\"", "\"kind\": \"official-free\"");
        let manifest = parse_manifest_json(&json).unwrap();
        let policy = validate_manifest(&manifest).unwrap();
        assert!(policy.official_repository_eligible);
        assert!(policy
            .warnings
            .iter()
            .any(|warning| warning.contains("source ToS")));
    }

    #[test]
    fn url_allow_list_is_exact_domain_only() {
        let manifest = parse_manifest_json(&valid_manifest_json("")).unwrap();
        assert!(is_url_allowed_by_manifest(
            &manifest,
            "https://www.aozora.gr.jp/cards/index.html"
        ));
        assert!(is_url_allowed_by_manifest(
            &manifest,
            "http://www.aozora.gr.jp:80/cards/index.html"
        ));
        assert!(!is_url_allowed_by_manifest(
            &manifest,
            "https://evil.www.aozora.gr.jp/cards/index.html"
        ));
        assert!(!is_url_allowed_by_manifest(
            &manifest,
            "file:///C:/secret.txt"
        ));
    }

    #[test]
    fn sdk_example_manifest_is_accepted_by_core_policy() {
        let json = include_str!("../../../plugin-sdk/examples/aozora-bunko/manifest.json");
        let manifest = parse_manifest_json(json).unwrap();
        assert_eq!(manifest.id, "aozora-bunko");
        assert!(
            validate_manifest(&manifest)
                .unwrap()
                .official_repository_eligible
        );
    }
}
