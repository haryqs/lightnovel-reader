use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::plugin_manifest::{ManifestValidation, PluginManifest};
use crate::plugin_package::load_plugin_package_zip;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallPreview {
    pub manifest: PluginManifest,
    pub validation: ManifestValidation,
    pub entry_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPlugin {
    pub manifest: PluginManifest,
    pub validation: ManifestValidation,
    pub entry_size: usize,
    pub installed_at: i64,
}

pub fn inspect_plugin_package(bytes: &[u8]) -> Result<PluginInstallPreview, String> {
    let package = load_plugin_package_zip(bytes)?;
    Ok(PluginInstallPreview {
        entry_size: package.entry_source.len(),
        manifest: package.manifest,
        validation: package.validation,
    })
}

pub fn install_plugin_package(
    plugin_root: &Path,
    bytes: &[u8],
    confirmed_user_legal: bool,
    installed_at: i64,
) -> Result<InstalledPlugin, String> {
    let package = load_plugin_package_zip(bytes)?;
    if package.validation.requires_user_legal_confirmation && !confirmed_user_legal {
        return Err("plugin requires explicit user legal confirmation".into());
    }

    let installed = InstalledPlugin {
        entry_size: package.entry_source.len(),
        installed_at,
        manifest: package.manifest.clone(),
        validation: package.validation.clone(),
    };

    let dir = plugin_dir(plugin_root, &package.manifest.id);
    fs::create_dir_all(&dir).map_err(|e| format!("create plugin directory failed: {e}"))?;

    let manifest_json = serde_json::to_string_pretty(&package.manifest)
        .map_err(|e| format!("serialize plugin manifest failed: {e}"))?;
    let install_json = serde_json::to_string_pretty(&installed)
        .map_err(|e| format!("serialize plugin install metadata failed: {e}"))?;

    fs::write(dir.join("manifest.json"), manifest_json)
        .map_err(|e| format!("write plugin manifest failed: {e}"))?;
    fs::write(dir.join(&package.manifest.entry), package.entry_source)
        .map_err(|e| format!("write plugin entry failed: {e}"))?;
    fs::write(dir.join("install.json"), install_json)
        .map_err(|e| format!("write plugin install metadata failed: {e}"))?;

    Ok(installed)
}

pub fn list_installed_plugins(plugin_root: &Path) -> Result<Vec<InstalledPlugin>, String> {
    if !plugin_root.exists() {
        return Ok(Vec::new());
    }

    let mut plugins = Vec::new();
    let entries =
        fs::read_dir(plugin_root).map_err(|e| format!("read plugin directory failed: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read plugin entry failed: {e}"))?;
        if !entry
            .file_type()
            .map_err(|e| format!("read plugin entry type failed: {e}"))?
            .is_dir()
        {
            continue;
        }
        let install_path = entry.path().join("install.json");
        if !install_path.exists() {
            continue;
        }
        let text = fs::read_to_string(&install_path)
            .map_err(|e| format!("read plugin install metadata failed: {e}"))?;
        let plugin: InstalledPlugin = serde_json::from_str(&text)
            .map_err(|e| format!("parse plugin install metadata failed: {e}"))?;
        plugins.push(plugin);
    }

    plugins.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    Ok(plugins)
}

fn plugin_dir(root: &Path, id: &str) -> PathBuf {
    root.join(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

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

    fn temp_plugin_root(name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "lightnovel-reader-plugin-store-test-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn inspects_plugin_package_without_writing() {
        let bytes = zip_bytes(&[
            ("manifest.json", &manifest("plugin.js", "public-domain")),
            ("plugin.js", "export default {}"),
        ]);
        let preview = inspect_plugin_package(&bytes).unwrap();
        assert_eq!(preview.manifest.id, "sample-source");
        assert_eq!(preview.entry_size, "export default {}".len());
        assert!(preview.validation.official_repository_eligible);
    }

    #[test]
    fn installs_plugin_package_files_and_metadata() {
        let root = temp_plugin_root("install");
        let bytes = zip_bytes(&[
            ("manifest.json", &manifest("plugin.js", "public-domain")),
            ("plugin.js", "export default {}"),
        ]);

        let installed = install_plugin_package(&root, &bytes, false, 42).unwrap();
        assert_eq!(installed.installed_at, 42);
        assert!(root.join("sample-source").join("manifest.json").exists());
        assert!(root.join("sample-source").join("plugin.js").exists());
        assert!(root.join("sample-source").join("install.json").exists());

        let plugins = list_installed_plugins(&root).unwrap();
        assert_eq!(plugins, vec![installed]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_user_declared_without_explicit_confirmation() {
        let root = temp_plugin_root("confirm");
        let bytes = zip_bytes(&[
            ("manifest.json", &manifest("plugin.js", "user-declared")),
            ("plugin.js", "export default {}"),
        ]);

        let err = install_plugin_package(&root, &bytes, false, 42).unwrap_err();
        assert!(err.contains("legal confirmation"));
        let installed = install_plugin_package(&root, &bytes, true, 43).unwrap();
        assert!(installed.validation.requires_user_legal_confirmation);
        let _ = fs::remove_dir_all(&root);
    }
}
