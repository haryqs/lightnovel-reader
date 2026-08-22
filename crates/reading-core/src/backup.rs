//! 用户数据备份：生成可校验、可迁移的独立目录。
//!
//! 两个 SQLite 数据库必须通过现有连接创建一致性快照，不能复制正在使用的数据库文件；
//! Tauri 等平台壳只负责选择目标目录和持有连接锁。

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const BACKUP_SCHEMA_VERSION: u32 = 1;
const BACKUP_PREFIX: &str = "lightnovel-reader-backup-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub schema_version: u32,
    pub app_version: String,
    pub created_at: i64,
    pub excluded: Vec<String>,
    pub files: Vec<BackupFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserDataBackupResult {
    pub path: String,
    pub created_at: i64,
    pub file_count: usize,
    pub total_bytes: u64,
}

/// 导出完整用户数据快照。目标必须是 app data 目录之外的既有目录。
pub fn export_user_data_backup(
    storage_db: &Connection,
    library_db: &Connection,
    app_data_dir: &Path,
    destination_parent: &Path,
    app_version: &str,
) -> Result<UserDataBackupResult, String> {
    if app_version.trim().is_empty() {
        return Err("应用版本不能为空".into());
    }

    let app_data_dir = app_data_dir
        .canonicalize()
        .map_err(|error| format!("无法解析应用数据目录: {error}"))?;
    let destination_parent = destination_parent
        .canonicalize()
        .map_err(|error| format!("无法解析备份目标目录: {error}"))?;
    if !destination_parent.is_dir() {
        return Err("备份目标必须是文件夹".into());
    }
    if path_is_within(&destination_parent, &app_data_dir) {
        return Err("备份目标不能位于应用数据目录内部".into());
    }

    let created_at = now_ms();
    let (partial_dir, final_dir) = unique_backup_paths(&destination_parent, created_at);
    fs::create_dir(&partial_dir).map_err(|error| format!("创建备份临时目录失败: {error}"))?;

    let result = build_backup(
        storage_db,
        library_db,
        &app_data_dir,
        &partial_dir,
        app_version,
        created_at,
    );
    let manifest = match result {
        Ok(manifest) => manifest,
        Err(error) => {
            let _ = fs::remove_dir_all(&partial_dir);
            return Err(error);
        }
    };

    if let Err(error) = fs::rename(&partial_dir, &final_dir) {
        let _ = fs::remove_dir_all(&partial_dir);
        return Err(format!("完成备份目录失败: {error}"));
    }

    Ok(UserDataBackupResult {
        path: display_path(&final_dir),
        created_at,
        file_count: manifest.files.len(),
        total_bytes: manifest.files.iter().map(|file| file.size).sum(),
    })
}

fn build_backup(
    storage_db: &Connection,
    library_db: &Connection,
    app_data_dir: &Path,
    partial_dir: &Path,
    app_version: &str,
    created_at: i64,
) -> Result<BackupManifest, String> {
    let mut files = Vec::new();

    snapshot_database(storage_db, &partial_dir.join("reader.db"))?;
    add_file_entry(partial_dir, &partial_dir.join("reader.db"), &mut files)?;

    let library_source = app_data_dir.join("library");
    if !library_source.is_dir() {
        return Err("书库目录不存在，无法生成完整备份".into());
    }
    let library_target = partial_dir.join("library");
    fs::create_dir(&library_target).map_err(|error| format!("创建备份书库目录失败: {error}"))?;
    snapshot_database(library_db, &library_target.join("library.sqlite"))?;
    add_file_entry(
        partial_dir,
        &library_target.join("library.sqlite"),
        &mut files,
    )?;
    copy_tree(
        &library_source,
        &library_target,
        partial_dir,
        &mut files,
        |relative| {
            !matches!(
                relative.file_name().and_then(|name| name.to_str()),
                Some("library.sqlite" | "library.sqlite-wal" | "library.sqlite-shm")
            )
        },
    )?;

    let plugins_source = app_data_dir.join("plugins");
    if plugins_source.is_dir() {
        copy_tree(
            &plugins_source,
            &partial_dir.join("plugins"),
            partial_dir,
            &mut files,
            |_| true,
        )?;
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = BackupManifest {
        schema_version: BACKUP_SCHEMA_VERSION,
        app_version: app_version.to_string(),
        created_at,
        excluded: vec![
            "cache/".into(),
            "sync.json".into(),
            "SQLite WAL/SHM files".into(),
        ],
        files,
    };
    verify_files(partial_dir, &manifest.files)?;

    let mut manifest_json = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("序列化备份清单失败: {error}"))?;
    manifest_json.push(b'\n');
    fs::write(partial_dir.join("manifest.json"), manifest_json)
        .map_err(|error| format!("写入备份清单失败: {error}"))?;
    Ok(manifest)
}

fn snapshot_database(connection: &Connection, destination: &Path) -> Result<(), String> {
    let destination = destination
        .to_str()
        .ok_or_else(|| "备份数据库路径不是有效 Unicode".to_string())?;
    connection
        .execute("VACUUM INTO ?1", [destination])
        .map_err(|error| format!("创建 SQLite 一致性快照失败: {error}"))?;
    Ok(())
}

fn copy_tree<F>(
    source: &Path,
    target: &Path,
    backup_root: &Path,
    files: &mut Vec<BackupFile>,
    include: F,
) -> Result<(), String>
where
    F: Fn(&Path) -> bool + Copy,
{
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| format!("读取备份源失败 {}: {error}", source.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("备份源包含符号链接，已拒绝: {}", source.display()));
    }
    fs::create_dir_all(target)
        .map_err(|error| format!("创建备份目录失败 {}: {error}", target.display()))?;

    let mut entries = fs::read_dir(source)
        .map_err(|error| format!("读取备份源目录失败 {}: {error}", source.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("枚举备份源目录失败 {}: {error}", source.display()))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let source_path = entry.path();
        let relative = source_path
            .strip_prefix(source)
            .map_err(|error| format!("计算备份相对路径失败: {error}"))?;
        if !include(relative) {
            continue;
        }
        let entry_metadata = fs::symlink_metadata(&source_path)
            .map_err(|error| format!("读取备份条目失败 {}: {error}", source_path.display()))?;
        if entry_metadata.file_type().is_symlink() {
            return Err(format!(
                "备份源包含符号链接，已拒绝: {}",
                source_path.display()
            ));
        }
        let target_path = target.join(entry.file_name());
        if entry_metadata.is_dir() {
            copy_tree(&source_path, &target_path, backup_root, files, include)?;
        } else if entry_metadata.is_file() {
            fs::copy(&source_path, &target_path).map_err(|error| {
                format!(
                    "复制备份文件失败 {} -> {}: {error}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
            add_file_entry(backup_root, &target_path, files)?;
        }
    }
    Ok(())
}

fn add_file_entry(
    backup_root: &Path,
    file_path: &Path,
    files: &mut Vec<BackupFile>,
) -> Result<(), String> {
    let metadata = fs::metadata(file_path)
        .map_err(|error| format!("读取备份文件信息失败 {}: {error}", file_path.display()))?;
    files.push(BackupFile {
        path: portable_relative_path(backup_root, file_path)?,
        size: metadata.len(),
        sha256: sha256_file(file_path)?,
    });
    Ok(())
}

fn verify_files(backup_root: &Path, files: &[BackupFile]) -> Result<(), String> {
    for file in files {
        let path = backup_root.join(Path::new(&file.path));
        let metadata = fs::metadata(&path)
            .map_err(|error| format!("复核备份文件失败 {}: {error}", path.display()))?;
        if metadata.len() != file.size || sha256_file(&path)? != file.sha256 {
            return Err(format!("备份文件校验失败: {}", file.path));
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("读取备份文件失败 {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("计算备份摘要失败 {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn portable_relative_path(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|error| format!("备份文件不在目标目录内: {error}"))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

#[cfg(windows)]
fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(network_path) = value.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{network_path}");
    }
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_string()
}

#[cfg(not(windows))]
fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn unique_backup_paths(destination_parent: &Path, created_at: i64) -> (PathBuf, PathBuf) {
    for suffix in 0_u32.. {
        let name = if suffix == 0 {
            format!("{BACKUP_PREFIX}-{created_at}")
        } else {
            format!("{BACKUP_PREFIX}-{created_at}-{suffix}")
        };
        let final_dir = destination_parent.join(&name);
        let partial_dir = destination_parent.join(format!(".{name}.partial"));
        if !final_dir.exists() && !partial_dir.exists() {
            return (partial_dir, final_dir);
        }
    }
    unreachable!("u32 backup suffix space exhausted")
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(windows)]
fn path_is_within(path: &Path, root: &Path) -> bool {
    let path = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect::<Vec<_>>();
    let root = root
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect::<Vec<_>>();
    path.len() >= root.len()
        && path
            .iter()
            .zip(root.iter())
            .all(|(left, right)| left == right)
}

#[cfg(not(windows))]
fn path_is_within(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "lightnovel-reader-backup-{label}-{}-{}",
                std::process::id(),
                nonce
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> (TestDir, Connection, Connection) {
        let source = TestDir::new("source");
        fs::create_dir_all(source.0.join("library/objects")).unwrap();
        fs::create_dir_all(source.0.join("library/covers")).unwrap();
        fs::create_dir_all(source.0.join("plugins/sources/demo")).unwrap();
        fs::create_dir_all(source.0.join("cache/parser")).unwrap();
        fs::write(source.0.join("library/objects/book.epub"), b"epub-data").unwrap();
        fs::write(source.0.join("library/covers/book.png"), b"cover-data").unwrap();
        fs::write(
            source.0.join("plugins/sources/demo/manifest.json"),
            br#"{"id":"demo"}"#,
        )
        .unwrap();
        fs::write(
            source.0.join("plugins/sources/demo/kv.json"),
            br#"{"page":"2"}"#,
        )
        .unwrap();
        fs::write(source.0.join("cache/parser/info.json"), b"cache").unwrap();
        fs::write(source.0.join("sync.json"), br#"{"token":"secret"}"#).unwrap();

        let storage = Connection::open(source.0.join("reader.db")).unwrap();
        storage
            .execute("CREATE TABLE reading_state (book_id TEXT PRIMARY KEY)", [])
            .unwrap();
        storage
            .execute("INSERT INTO reading_state VALUES (?1)", params!["book-1"])
            .unwrap();
        let library = Connection::open(source.0.join("library/library.sqlite")).unwrap();
        library
            .execute("CREATE TABLE books (id TEXT PRIMARY KEY)", [])
            .unwrap();
        library
            .execute("INSERT INTO books VALUES (?1)", params!["book-1"])
            .unwrap();
        (source, storage, library)
    }

    #[test]
    fn exports_consistent_databases_assets_plugins_and_manifest() {
        let (source, storage, library) = fixture();
        let destination = TestDir::new("destination");
        let result =
            export_user_data_backup(&storage, &library, &source.0, &destination.0, "0.7.3")
                .unwrap();
        let backup = PathBuf::from(&result.path);

        assert!(backup.join("reader.db").is_file());
        assert!(backup.join("library/library.sqlite").is_file());
        assert_eq!(
            fs::read(backup.join("library/objects/book.epub")).unwrap(),
            b"epub-data"
        );
        assert!(backup.join("plugins/sources/demo/manifest.json").is_file());
        assert!(backup.join("plugins/sources/demo/kv.json").is_file());
        assert!(!backup.join("cache").exists());
        assert!(!backup.join("sync.json").exists());
        assert!(!backup.join("library/library.sqlite-wal").exists());

        let storage_snapshot = Connection::open(backup.join("reader.db")).unwrap();
        let storage_id: String = storage_snapshot
            .query_row("SELECT book_id FROM reading_state", [], |row| row.get(0))
            .unwrap();
        assert_eq!(storage_id, "book-1");
        let library_snapshot = Connection::open(backup.join("library/library.sqlite")).unwrap();
        let library_id: String = library_snapshot
            .query_row("SELECT id FROM books", [], |row| row.get(0))
            .unwrap();
        assert_eq!(library_id, "book-1");

        let manifest: BackupManifest =
            serde_json::from_slice(&fs::read(backup.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest.schema_version, BACKUP_SCHEMA_VERSION);
        assert_eq!(manifest.app_version, "0.7.3");
        assert_eq!(manifest.files.len(), result.file_count);
        assert_eq!(
            manifest.files.iter().map(|file| file.size).sum::<u64>(),
            result.total_bytes
        );
        verify_files(&backup, &manifest.files).unwrap();
        assert_eq!(
            fs::read(source.0.join("sync.json")).unwrap(),
            br#"{"token":"secret"}"#
        );
    }

    #[test]
    fn rejects_destination_inside_app_data() {
        let (source, storage, library) = fixture();
        let destination = source.0.join("exports");
        fs::create_dir(&destination).unwrap();

        let error = export_user_data_backup(&storage, &library, &source.0, &destination, "0.7.3")
            .unwrap_err();

        assert!(error.contains("不能位于应用数据目录内部"));
        assert!(fs::read_dir(destination).unwrap().next().is_none());
    }

    #[test]
    fn failure_removes_partial_directory() {
        let source = TestDir::new("incomplete-source");
        let storage = Connection::open_in_memory().unwrap();
        let library = Connection::open_in_memory().unwrap();
        let destination = TestDir::new("failure");

        let error = export_user_data_backup(&storage, &library, &source.0, &destination.0, "0.7.3")
            .unwrap_err();
        assert!(error.contains("书库目录不存在"));
        assert!(fs::read_dir(&destination.0).unwrap().next().is_none());
    }
}
