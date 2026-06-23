//! Official source-plugin repository index DTOs and validation.
//!
//! This module does not download, install, or execute plugins. It validates the
//! metadata that an official allow-list repository may publish before the shell
//! downloads a zip package and feeds it into `plugin_store`.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::plugin_manifest::{
    validate_manifest, ManifestValidation, PluginCapability, PluginLegalKind, PluginManifest,
    SUPPORTED_API_VERSION,
};

pub const SUPPORTED_REPOSITORY_SCHEMA_VERSION: &str = "0.1";
pub const MAX_REPOSITORY_ENTRIES: usize = 500;
pub const MAX_PACKAGE_SIZE_BYTES: u64 = 50 * 1024 * 1024;
pub const SUPPORTED_SIGNATURE_ALGORITHM: &str = "ed25519";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRepositoryIndex {
    pub schema_version: String,
    #[serde(default)]
    pub generated_at: Option<i64>,
    pub entries: Vec<PluginRepositoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRepositoryEntry {
    pub manifest: PluginManifest,
    pub package_url: String,
    pub package_sha256: String,
    #[serde(default)]
    pub package_size: Option<u64>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub signature: Option<PluginPackageSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPackageSignature {
    pub algorithm: String,
    pub key_id: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRepositoryValidation {
    pub entries: usize,
    pub warnings: Vec<String>,
}

pub fn parse_repository_index(json: &str) -> Result<PluginRepositoryIndex, String> {
    let index: PluginRepositoryIndex =
        serde_json::from_str(json).map_err(|e| format!("invalid plugin repository JSON: {e}"))?;
    validate_repository_index(&index)?;
    Ok(index)
}

pub fn validate_repository_index(
    index: &PluginRepositoryIndex,
) -> Result<PluginRepositoryValidation, String> {
    if index.schema_version != SUPPORTED_REPOSITORY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported plugin repository schemaVersion {}, expected {}",
            index.schema_version, SUPPORTED_REPOSITORY_SCHEMA_VERSION
        ));
    }
    if index.entries.len() > MAX_REPOSITORY_ENTRIES {
        return Err(format!(
            "plugin repository entries exceed limit {MAX_REPOSITORY_ENTRIES}"
        ));
    }

    let mut ids = BTreeSet::new();
    let mut warnings = Vec::new();
    for entry in &index.entries {
        validate_repository_entry(entry, &mut warnings)?;
        if !ids.insert(entry.manifest.id.clone()) {
            return Err(format!(
                "plugin repository contains duplicate plugin id: {}",
                entry.manifest.id
            ));
        }
    }

    Ok(PluginRepositoryValidation {
        entries: index.entries.len(),
        warnings,
    })
}

pub fn verify_package_sha256(bytes: &[u8], expected_hex: &str) -> Result<(), String> {
    if !is_sha256_hex(expected_hex) {
        return Err("plugin package sha256 must be 64 hex characters".into());
    }
    let actual = Sha256::digest(bytes);
    let actual_hex: String = actual.iter().map(|b| format!("{:02x}", b)).collect();
    if !actual_hex.eq_ignore_ascii_case(expected_hex) {
        return Err("plugin package sha256 mismatch".into());
    }
    Ok(())
}

fn validate_repository_entry(
    entry: &PluginRepositoryEntry,
    warnings: &mut Vec<String>,
) -> Result<ManifestValidation, String> {
    let manifest_validation = validate_manifest(&entry.manifest)
        .map_err(|e| format!("invalid manifest for {}: {e}", entry.manifest.id))?;
    if entry.manifest.api_version != SUPPORTED_API_VERSION {
        return Err(format!(
            "plugin {} apiVersion is not supported by this repository",
            entry.manifest.id
        ));
    }
    if !manifest_validation.official_repository_eligible {
        return Err(format!(
            "plugin {} is not eligible for the official repository",
            entry.manifest.id
        ));
    }
    if entry.manifest.legal.kind == PluginLegalKind::OfficialFree
        && entry
            .manifest
            .capabilities
            .contains(&PluginCapability::Acquire)
    {
        return Err(format!(
            "plugin {} cannot publish official-free acquire in the official repository before ToS gates exist",
            entry.manifest.id
        ));
    }
    if !is_https_url(&entry.package_url) {
        return Err(format!(
            "plugin {} packageUrl must be https",
            entry.manifest.id
        ));
    }
    if !is_sha256_hex(&entry.package_sha256) {
        return Err(format!(
            "plugin {} packageSha256 must be 64 hex characters",
            entry.manifest.id
        ));
    }
    if let Some(size) = entry.package_size {
        if size == 0 || size > MAX_PACKAGE_SIZE_BYTES {
            return Err(format!(
                "plugin {} packageSize must be 1..={MAX_PACKAGE_SIZE_BYTES}",
                entry.manifest.id
            ));
        }
    }
    if let Some(source_url) = &entry.source_url {
        if !is_https_url(source_url) {
            return Err(format!(
                "plugin {} sourceUrl must be https",
                entry.manifest.id
            ));
        }
    }
    if let Some(signature) = &entry.signature {
        validate_signature(&entry.manifest.id, signature)?;
        warnings.push(format!(
            "plugin {} has signature metadata; cryptographic verification is not implemented yet",
            entry.manifest.id
        ));
    }
    for warning in manifest_validation.warnings.iter().cloned() {
        warnings.push(format!("plugin {}: {warning}", entry.manifest.id));
    }
    Ok(manifest_validation)
}

fn validate_signature(plugin_id: &str, signature: &PluginPackageSignature) -> Result<(), String> {
    if signature.algorithm != SUPPORTED_SIGNATURE_ALGORITHM {
        return Err(format!(
            "plugin {plugin_id} signature algorithm must be {SUPPORTED_SIGNATURE_ALGORITHM}"
        ));
    }
    if signature.key_id.trim().is_empty() || signature.key_id.chars().count() > 128 {
        return Err(format!("plugin {plugin_id} signature keyId is invalid"));
    }
    if signature.value.trim().is_empty() || signature.value.chars().count() > 512 {
        return Err(format!("plugin {plugin_id} signature value is invalid"));
    }
    Ok(())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_https_url(value: &str) -> bool {
    value
        .strip_prefix("https://")
        .is_some_and(|rest| rest.contains('.') && !rest.contains(char::is_whitespace))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA256_EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn manifest(id: &str, legal_kind: &str, capabilities: &str) -> String {
        format!(
            r#"{{
  "apiVersion": "0.1",
  "id": "{id}",
  "name": "Sample Source",
  "version": "0.1.0",
  "entry": "plugin.js",
  "domains": ["example.org"],
  "permissions": ["http"],
  "capabilities": {capabilities},
  "legal": {{ "kind": "{legal_kind}" }}
}}"#
        )
    }

    fn index(entries: &str) -> String {
        format!(
            r#"{{
  "schemaVersion": "0.1",
  "generatedAt": 1760000000000,
  "entries": [{entries}]
}}"#
        )
    }

    fn entry(id: &str, legal_kind: &str, capabilities: &str, sha: &str) -> String {
        format!(
            r#"{{
  "manifest": {},
  "packageUrl": "https://plugins.example.org/{id}.zip",
  "packageSha256": "{sha}",
  "packageSize": 1234,
  "sourceUrl": "https://github.com/example/{id}"
}}"#,
            manifest(id, legal_kind, capabilities)
        )
    }

    #[test]
    fn parses_valid_repository_index() {
        let json = index(&entry(
            "aozora-bunko",
            "public-domain",
            r#"["browse", "acquire"]"#,
            SHA256_EMPTY,
        ));
        let parsed = parse_repository_index(&json).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        let validation = validate_repository_index(&parsed).unwrap();
        assert_eq!(validation.entries, 1);
    }

    #[test]
    fn rejects_user_declared_plugins() {
        let json = index(&entry(
            "private-source",
            "user-declared",
            "[]",
            SHA256_EMPTY,
        ));
        let err = parse_repository_index(&json).unwrap_err();
        assert!(err.contains("not eligible"));
    }

    #[test]
    fn rejects_duplicate_plugin_ids() {
        let item = entry("aozora-bunko", "public-domain", "[]", SHA256_EMPTY);
        let json = index(&format!("{item},{item}"));
        let err = parse_repository_index(&json).unwrap_err();
        assert!(err.contains("duplicate plugin id"));
    }

    #[test]
    fn rejects_non_https_package_and_source_urls() {
        let json = index(
            &entry("aozora-bunko", "public-domain", "[]", SHA256_EMPTY)
                .replace("https://plugins.example.org", "http://plugins.example.org"),
        );
        assert!(parse_repository_index(&json)
            .unwrap_err()
            .contains("packageUrl"));

        let json = index(
            &entry("aozora-bunko", "public-domain", "[]", SHA256_EMPTY)
                .replace("https://github.com", "http://github.com"),
        );
        assert!(parse_repository_index(&json)
            .unwrap_err()
            .contains("sourceUrl"));
    }

    #[test]
    fn rejects_invalid_hash_and_package_size() {
        let json = index(&entry("aozora-bunko", "public-domain", "[]", "abc"));
        assert!(parse_repository_index(&json)
            .unwrap_err()
            .contains("packageSha256"));

        let json = index(
            &entry("aozora-bunko", "public-domain", "[]", SHA256_EMPTY)
                .replace("\"packageSize\": 1234", "\"packageSize\": 0"),
        );
        assert!(parse_repository_index(&json)
            .unwrap_err()
            .contains("packageSize"));
    }

    #[test]
    fn rejects_official_free_acquire_until_tos_gate_exists() {
        let json = index(&entry(
            "narou-source",
            "official-free",
            r#"["acquire"]"#,
            SHA256_EMPTY,
        ));
        let err = parse_repository_index(&json).unwrap_err();
        assert!(err.contains("official-free acquire"));
    }

    #[test]
    fn accepts_signature_metadata_shape_but_warns_not_verified() {
        let json = index(
            &entry("aozora-bunko", "public-domain", "[]", SHA256_EMPTY).replace(
                r#""sourceUrl": "https://github.com/example/aozora-bunko""#,
                r#""sourceUrl": "https://github.com/example/aozora-bunko",
  "signature": { "algorithm": "ed25519", "keyId": "official-2026", "value": "abc" }"#,
            ),
        );
        let parsed = parse_repository_index(&json).unwrap();
        let validation = validate_repository_index(&parsed).unwrap();
        assert!(validation
            .warnings
            .iter()
            .any(|warning| warning.contains("not implemented yet")));
    }

    #[test]
    fn verifies_package_sha256() {
        verify_package_sha256(b"", SHA256_EMPTY).unwrap();
        assert!(verify_package_sha256(b"x", SHA256_EMPTY)
            .unwrap_err()
            .contains("mismatch"));
        assert!(verify_package_sha256(b"", "abc")
            .unwrap_err()
            .contains("64 hex"));
    }
}
