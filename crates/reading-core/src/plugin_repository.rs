//! Official source-plugin repository index DTOs and validation.
//!
//! This module does not download, install, or execute plugins. It validates the
//! metadata that an official allow-list repository may publish before the shell
//! downloads a zip package and feeds it into `plugin_store`.

use std::collections::BTreeSet;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use ring::signature::{UnparsedPublicKey, ED25519};
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
pub const ED25519_PUBLIC_KEY_BYTES: usize = 32;
pub const ED25519_SIGNATURE_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustedPluginKey<'a> {
    pub key_id: &'a str,
    /// Base64-encoded raw 32-byte Ed25519 public key.
    pub public_key_base64: &'a str,
}

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

/// Apply the desktop shell's compiled trust policy to a structurally valid index.
/// Signatures cover the downloaded zip bytes, so the actual cryptographic check is
/// repeated by `verify_package_signature` after every preview/install download.
pub fn validate_repository_index_with_keyring(
    index: &PluginRepositoryIndex,
    trusted_keys: &[TrustedPluginKey<'_>],
    require_signatures: bool,
) -> Result<PluginRepositoryValidation, String> {
    let mut validation = validate_repository_index(index)?;
    validate_trusted_keyring(trusted_keys)?;

    for entry in &index.entries {
        match &entry.signature {
            Some(signature) => {
                trusted_key_bytes(trusted_keys, &signature.key_id)?;
            }
            None if require_signatures => {
                return Err(format!(
                    "plugin {} package signature is required by repository policy",
                    entry.manifest.id
                ));
            }
            None => validation.warnings.push(format!(
                "plugin {} package is unsigned; manual allow-list review is required",
                entry.manifest.id
            )),
        }
    }
    Ok(validation)
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

/// Verify an Ed25519 signature over the exact plugin zip bytes.
pub fn verify_package_signature(
    bytes: &[u8],
    signature: &PluginPackageSignature,
    trusted_keys: &[TrustedPluginKey<'_>],
) -> Result<(), String> {
    validate_signature("package", signature)?;
    validate_trusted_keyring(trusted_keys)?;
    let public_key = trusted_key_bytes(trusted_keys, &signature.key_id)?;
    let signature_bytes = decode_base64_exact(
        &signature.value,
        ED25519_SIGNATURE_BYTES,
        "plugin package signature value",
    )?;
    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(bytes, &signature_bytes)
        .map_err(|_| {
            format!(
                "plugin package signature verification failed for keyId {}",
                signature.key_id
            )
        })
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
            "plugin {} cannot publish official-free acquire without source-specific authorization review",
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
    decode_base64_exact(
        &signature.value,
        ED25519_SIGNATURE_BYTES,
        &format!("plugin {plugin_id} signature value"),
    )?;
    Ok(())
}

fn validate_trusted_keyring(trusted_keys: &[TrustedPluginKey<'_>]) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    for key in trusted_keys {
        if key.key_id.trim().is_empty() || key.key_id.chars().count() > 128 {
            return Err("trusted plugin keyId is invalid".into());
        }
        if !ids.insert(key.key_id) {
            return Err(format!("duplicate trusted plugin keyId: {}", key.key_id));
        }
        decode_base64_exact(
            key.public_key_base64,
            ED25519_PUBLIC_KEY_BYTES,
            &format!("trusted plugin public key {}", key.key_id),
        )?;
    }
    Ok(())
}

fn trusted_key_bytes(
    trusted_keys: &[TrustedPluginKey<'_>],
    key_id: &str,
) -> Result<Vec<u8>, String> {
    let key = trusted_keys
        .iter()
        .find(|key| key.key_id == key_id)
        .ok_or_else(|| format!("plugin package signature uses unknown keyId: {key_id}"))?;
    decode_base64_exact(
        key.public_key_base64,
        ED25519_PUBLIC_KEY_BYTES,
        &format!("trusted plugin public key {key_id}"),
    )
}

fn decode_base64_exact(value: &str, expected_len: usize, label: &str) -> Result<Vec<u8>, String> {
    let decoded = BASE64_STANDARD
        .decode(value.trim())
        .map_err(|_| format!("{label} must be valid base64"))?;
    if decoded.len() != expected_len {
        return Err(format!("{label} must decode to {expected_len} bytes"));
    }
    Ok(decoded)
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
    use ring::signature::{Ed25519KeyPair, KeyPair};

    const SHA256_EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn manifest(id: &str, legal_kind: &str, capabilities: &str) -> String {
        let terms = if legal_kind == "official-free" {
            r#", "termsUrl": "https://example.org/terms""#
        } else {
            ""
        };
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
  "legal": {{ "kind": "{legal_kind}"{terms} }}
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
    fn rejects_official_free_acquire_without_source_specific_review() {
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
    fn accepts_valid_signature_metadata_shape() {
        let signature_value = BASE64_STANDARD.encode([0_u8; ED25519_SIGNATURE_BYTES]);
        let json = index(
            &entry("aozora-bunko", "public-domain", "[]", SHA256_EMPTY).replace(
                r#""sourceUrl": "https://github.com/example/aozora-bunko""#,
                &format!(r#""sourceUrl": "https://github.com/example/aozora-bunko",
  "signature": {{ "algorithm": "ed25519", "keyId": "official-2026", "value": "{signature_value}" }}"#),
            ),
        );
        let parsed = parse_repository_index(&json).unwrap();
        let validation = validate_repository_index(&parsed).unwrap();
        assert!(validation.warnings.is_empty());
    }

    #[test]
    fn verifies_signed_package_bytes_with_trusted_keyring() {
        let package = b"signed plugin zip fixture";
        let seed = decode_hex_32(
            "9d61b19deffd5a60ba844af492ec2cc4\
             4449c5697b326919703bac031cae7f60",
        );
        let public_key = decode_hex_32(
            "d75a980182b10ab7d54bfed3c964073a\
             0ee172f3daa62325af021a68f707511a",
        );
        let key_pair = Ed25519KeyPair::from_seed_and_public_key(&seed, &public_key).unwrap();
        assert_eq!(key_pair.public_key().as_ref(), public_key);
        let signature = PluginPackageSignature {
            algorithm: "ed25519".into(),
            key_id: "official-test".into(),
            value: BASE64_STANDARD.encode(key_pair.sign(package).as_ref()),
        };
        let public_key_base64 = BASE64_STANDARD.encode(public_key);
        let keys = [TrustedPluginKey {
            key_id: "official-test",
            public_key_base64: &public_key_base64,
        }];

        verify_package_signature(package, &signature, &keys).unwrap();
        assert!(verify_package_signature(b"tampered", &signature, &keys)
            .unwrap_err()
            .contains("verification failed"));

        let mut parsed = parse_repository_index(&index(&entry(
            "aozora-bunko",
            "public-domain",
            "[]",
            SHA256_EMPTY,
        )))
        .unwrap();
        parsed.entries[0].signature = Some(signature.clone());
        validate_repository_index_with_keyring(&parsed, &keys, true).unwrap();

        parsed.entries[0].signature.as_mut().unwrap().key_id = "unknown".into();
        assert!(validate_repository_index_with_keyring(&parsed, &keys, true)
            .unwrap_err()
            .contains("unknown keyId"));
    }

    #[test]
    fn unsigned_repository_requires_manual_mode() {
        let parsed = parse_repository_index(&index(&entry(
            "aozora-bunko",
            "public-domain",
            "[]",
            SHA256_EMPTY,
        )))
        .unwrap();
        let validation = validate_repository_index_with_keyring(&parsed, &[], false).unwrap();
        assert!(validation
            .warnings
            .iter()
            .any(|warning| warning.contains("manual allow-list review")));
        assert!(validate_repository_index_with_keyring(&parsed, &[], true)
            .unwrap_err()
            .contains("signature is required"));
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

    fn decode_hex_32(value: &str) -> [u8; 32] {
        let compact = value
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>();
        assert_eq!(compact.len(), 64);
        let mut out = [0_u8; 32];
        for (index, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&compact[index * 2..index * 2 + 2], 16).unwrap();
        }
        out
    }
}
