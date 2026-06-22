use std::io::{Cursor, Read};

use crate::plugin_manifest::{
    parse_manifest_json, validate_manifest, ManifestValidation, PluginManifest,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPackage {
    pub manifest: PluginManifest,
    pub validation: ManifestValidation,
    pub entry_source: String,
}

pub fn load_plugin_package_zip(bytes: &[u8]) -> Result<PluginPackage, String> {
    if bytes.is_empty() {
        return Err("plugin package is empty".into());
    }

    let cursor = Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("invalid plugin package zip: {e}"))?;

    let manifest_path = find_manifest_path(&mut archive)?;
    let manifest_json = read_zip_text(&mut archive, &manifest_path, "manifest")?;
    let manifest = parse_manifest_json(&manifest_json)?;
    let validation = validate_manifest(&manifest)?;

    let entry_path = package_relative_path(&manifest_path, &manifest.entry)?;
    let entry_source = read_zip_text(&mut archive, &entry_path, "entry script")?;
    if entry_source.trim().is_empty() {
        return Err("plugin entry script is empty".into());
    }

    Ok(PluginPackage {
        manifest,
        validation,
        entry_source,
    })
}

fn find_manifest_path<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<String, String> {
    let mut found = None;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = normalize_zip_path(entry.name())?;
        if name.ends_with("/manifest.json") || name == "manifest.json" {
            if found.is_some() {
                return Err("plugin package contains multiple manifest.json files".into());
            }
            found = Some(name);
        }
    }
    found.ok_or_else(|| "plugin package is missing manifest.json".into())
}

fn read_zip_text<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    path: &str,
    label: &str,
) -> Result<String, String> {
    let normalized = normalize_zip_path(path)?;
    let mut entry = archive
        .by_name(&normalized)
        .map_err(|_| format!("plugin package is missing {label}: {normalized}"))?;
    if entry.is_dir() {
        return Err(format!("plugin {label} is a directory: {normalized}"));
    }

    let mut buf = Vec::new();
    entry
        .read_to_end(&mut buf)
        .map_err(|e| format!("read plugin {label} failed: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("plugin {label} must be UTF-8: {e}"))
}

fn package_relative_path(manifest_path: &str, entry_name: &str) -> Result<String, String> {
    let entry = normalize_entry_name(entry_name)?;
    let prefix = manifest_path.rsplit_once('/').map(|(prefix, _)| prefix);
    Ok(match prefix {
        Some(prefix) if !prefix.is_empty() => format!("{prefix}/{entry}"),
        _ => entry,
    })
}

fn normalize_entry_name(entry_name: &str) -> Result<String, String> {
    let normalized = normalize_zip_path(entry_name)?;
    if normalized.contains('/') || !normalized.ends_with(".js") || normalized == ".js" {
        return Err("plugin entry must be a single .js file name".into());
    }
    Ok(normalized)
}

fn normalize_zip_path(path: &str) -> Result<String, String> {
    let path = path.replace('\\', "/");
    if path.starts_with('/') || path.contains('\0') {
        return Err(format!("unsafe plugin package path: {path}"));
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(format!("unsafe plugin package path: {path}"));
        }
        parts.push(part);
    }
    if parts.is_empty() {
        return Err("empty plugin package path".into());
    }
    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn manifest(entry: &str, legal_kind: &str) -> String {
        format!(
            r#"{{
  "apiVersion": "0.1",
  "id": "sample-source",
  "name": "Sample Source",
  "version": "0.1.0",
  "entry": "{entry}",
  "domains": ["example.org"],
  "permissions": ["http"],
  "capabilities": ["browse"],
  "legal": {{ "kind": "{legal_kind}" }}
}}"#
        )
    }

    fn zip_bytes(files: &[(&str, &str)]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (path, text) in files {
            writer.start_file(*path, options).unwrap();
            writer.write_all(text.as_bytes()).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn loads_root_plugin_package() {
        let bytes = zip_bytes(&[
            ("manifest.json", &manifest("plugin.js", "public-domain")),
            ("plugin.js", "export default {}"),
        ]);
        let package = load_plugin_package_zip(&bytes).unwrap();
        assert_eq!(package.manifest.id, "sample-source");
        assert_eq!(package.entry_source, "export default {}");
        assert!(package.validation.official_repository_eligible);
    }

    #[test]
    fn loads_nested_single_directory_package() {
        let bytes = zip_bytes(&[
            (
                "sample/manifest.json",
                &manifest("plugin.js", "public-domain"),
            ),
            ("sample/plugin.js", "export default { search() {} }"),
        ]);
        let package = load_plugin_package_zip(&bytes).unwrap();
        assert_eq!(package.manifest.entry, "plugin.js");
        assert!(package.entry_source.contains("search"));
    }

    #[test]
    fn rejects_missing_entry_script() {
        let bytes = zip_bytes(&[("manifest.json", &manifest("plugin.js", "public-domain"))]);
        let err = load_plugin_package_zip(&bytes).unwrap_err();
        assert!(err.contains("missing entry script"));
    }

    #[test]
    fn rejects_multiple_manifests() {
        let bytes = zip_bytes(&[
            ("manifest.json", &manifest("plugin.js", "public-domain")),
            (
                "other/manifest.json",
                &manifest("plugin.js", "public-domain"),
            ),
            ("plugin.js", "export default {}"),
        ]);
        let err = load_plugin_package_zip(&bytes).unwrap_err();
        assert!(err.contains("multiple manifest"));
    }

    #[test]
    fn rejects_entry_path_traversal_even_if_zip_contains_it() {
        let bytes = zip_bytes(&[
            ("manifest.json", &manifest("../plugin.js", "public-domain")),
            ("plugin.js", "export default {}"),
        ]);
        let err = load_plugin_package_zip(&bytes).unwrap_err();
        assert!(err.contains("entry"));
    }

    #[test]
    fn preserves_user_declared_policy_flags() {
        let bytes = zip_bytes(&[
            ("manifest.json", &manifest("plugin.js", "user-declared")),
            ("plugin.js", "export default {}"),
        ]);
        let package = load_plugin_package_zip(&bytes).unwrap();
        assert!(!package.validation.official_repository_eligible);
        assert!(package.validation.requires_user_legal_confirmation);
    }

    #[test]
    fn rejects_non_zip_bytes() {
        let err = load_plugin_package_zip(b"not a zip").unwrap_err();
        assert!(err.contains("invalid plugin package zip"));
    }
}
