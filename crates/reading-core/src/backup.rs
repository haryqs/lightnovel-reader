//! 用户数据备份：生成可校验、可迁移的独立目录。
//!
//! 两个 SQLite 数据库必须通过现有连接创建一致性快照，不能复制正在使用的数据库文件；
//! Tauri 等平台壳只负责选择目标目录和持有连接锁。

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const BACKUP_SCHEMA_VERSION: u32 = 1;
const BACKUP_PREFIX: &str = "lightnovel-reader-backup-v1";
const MAX_MANIFEST_BYTES: u64 = 8 * 1024 * 1024;
const MAX_BACKUP_FILES: usize = 200_000;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserDataBackupInspection {
    pub path: String,
    pub schema_version: u32,
    pub source_app_version: String,
    pub created_at: i64,
    pub file_count: usize,
    pub total_bytes: u64,
    pub library_book_count: u64,
    pub reading_progress_count: u64,
    pub annotation_count: u64,
    pub plugin_count: usize,
    pub epub_file_count: usize,
    pub newer_than_current_app: bool,
    pub warnings: Vec<String>,
}

/// 只读检查备份目录，验证清单、文件集合、摘要与 SQLite 完整性，并返回内容预览。
/// 此函数不得创建、修改、移动或删除备份及当前应用数据。
pub fn inspect_user_data_backup(
    backup_dir: &Path,
    current_app_version: &str,
) -> Result<UserDataBackupInspection, String> {
    if current_app_version.trim().is_empty() {
        return Err("当前应用版本不能为空".into());
    }
    let root_metadata =
        fs::symlink_metadata(backup_dir).map_err(|error| format!("读取备份目录失败: {error}"))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err("备份路径必须是普通文件夹，不能是符号链接".into());
    }
    let backup_dir = backup_dir
        .canonicalize()
        .map_err(|error| format!("无法解析备份目录: {error}"))?;
    let manifest_path = backup_dir.join("manifest.json");
    let manifest_metadata = fs::symlink_metadata(&manifest_path)
        .map_err(|error| format!("备份缺少 manifest.json: {error}"))?;
    if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
        return Err("manifest.json 必须是普通文件".into());
    }
    if manifest_metadata.len() > MAX_MANIFEST_BYTES {
        return Err(format!(
            "manifest.json 超过 {} MiB 安全上限",
            MAX_MANIFEST_BYTES / 1024 / 1024
        ));
    }
    let manifest: BackupManifest = serde_json::from_slice(
        &fs::read(&manifest_path).map_err(|error| format!("读取 manifest.json 失败: {error}"))?,
    )
    .map_err(|error| format!("解析 manifest.json 失败: {error}"))?;
    validate_manifest(&manifest)?;

    let declared = manifest
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let actual = collect_payload_files(&backup_dir, &backup_dir)?;
    if declared != actual {
        let missing = declared.difference(&actual).next();
        let extra = actual.difference(&declared).next();
        return Err(match (missing, extra) {
            (Some(path), _) => format!("备份清单中的文件不存在: {path}"),
            (_, Some(path)) => format!("备份包含清单外文件: {path}"),
            _ => "备份文件集合与清单不一致".into(),
        });
    }
    verify_files(&backup_dir, &manifest.files)?;

    let reader_db = backup_dir.join("reader.db");
    let library_db = backup_dir.join("library/library.sqlite");
    let reader = open_verified_database(&reader_db, "阅读数据")?;
    let library = open_verified_database(&library_db, "书库")?;
    let library_book_count = if table_exists(&library, "edition")? {
        table_count(&library, "edition")?
    } else {
        table_count(&library, "books")?
    };
    let reading_progress_count = table_count(&reader, "reading_state")?;
    let annotation_count = table_count(&reader, "annotations")?;

    let plugin_ids = manifest
        .files
        .iter()
        .filter_map(|file| {
            let parts = file.path.split('/').collect::<Vec<_>>();
            (parts.len() == 4
                && parts[0] == "plugins"
                && parts[1] == "sources"
                && matches!(parts[3], "install.json" | "manifest.json"))
            .then(|| parts[2].to_string())
        })
        .collect::<BTreeSet<_>>();
    let epub_file_count = manifest
        .files
        .iter()
        .filter(|file| file.path.to_ascii_lowercase().ends_with(".epub"))
        .count();
    let total_bytes = manifest.files.iter().try_fold(0_u64, |total, file| {
        total
            .checked_add(file.size)
            .ok_or_else(|| "备份文件总大小溢出".to_string())
    })?;
    let mut warnings = Vec::new();
    let newer_than_current_app = match (
        numeric_version(&manifest.app_version),
        numeric_version(current_app_version),
    ) {
        (Some(source), Some(current)) => source > current,
        _ => {
            warnings.push("无法精确比较备份与当前应用版本，请谨慎恢复".into());
            false
        }
    };
    if newer_than_current_app {
        warnings.push(format!(
            "备份来自较新的应用版本 v{}；当前 v{} 仅支持校验预览",
            manifest.app_version, current_app_version
        ));
    }

    Ok(UserDataBackupInspection {
        path: display_path(&backup_dir),
        schema_version: manifest.schema_version,
        source_app_version: manifest.app_version,
        created_at: manifest.created_at,
        file_count: manifest.files.len(),
        total_bytes,
        library_book_count,
        reading_progress_count,
        annotation_count,
        plugin_count: plugin_ids.len(),
        epub_file_count,
        newer_than_current_app,
        warnings,
    })
}

fn validate_manifest(manifest: &BackupManifest) -> Result<(), String> {
    if manifest.schema_version != BACKUP_SCHEMA_VERSION {
        return Err(format!(
            "不支持的备份 schemaVersion {}（当前仅支持 {}）",
            manifest.schema_version, BACKUP_SCHEMA_VERSION
        ));
    }
    if manifest.app_version.trim().is_empty() || manifest.created_at <= 0 {
        return Err("备份清单缺少有效的应用版本或创建时间".into());
    }
    if manifest.files.len() > MAX_BACKUP_FILES {
        return Err(format!("备份文件数超过 {MAX_BACKUP_FILES} 个安全上限"));
    }
    let mut paths = BTreeSet::new();
    for file in &manifest.files {
        validate_payload_path(&file.path)?;
        if !paths.insert(file.path.clone()) {
            return Err(format!("备份清单包含重复路径: {}", file.path));
        }
        if file.sha256.len() != 64 || !file.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("备份摘要格式无效: {}", file.path));
        }
    }
    for required in ["reader.db", "library/library.sqlite"] {
        if !paths.contains(required) {
            return Err(format!("备份缺少必要文件: {required}"));
        }
    }
    Ok(())
}

fn validate_payload_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.contains('\\')
        || path.contains(':')
        || path.starts_with('/')
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err(format!("备份清单包含不安全路径: {path}"));
    }
    let lower = path.to_ascii_lowercase();
    if lower == "manifest.json"
        || lower == "sync.json"
        || lower.starts_with("cache/")
        || lower.ends_with("-wal")
        || lower.ends_with("-shm")
    {
        return Err(format!("备份清单包含禁止载荷: {path}"));
    }
    Ok(())
}

fn collect_payload_files(root: &Path, directory: &Path) -> Result<BTreeSet<String>, String> {
    let mut result = BTreeSet::new();
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("读取备份目录失败 {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("枚举备份目录失败 {}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("读取备份条目失败 {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("备份包含符号链接，已拒绝: {}", path.display()));
        }
        if metadata.is_dir() {
            result.extend(collect_payload_files(root, &path)?);
        } else if metadata.is_file() {
            let relative = portable_relative_path(root, &path)?;
            if relative != "manifest.json" {
                result.insert(relative);
            }
        } else {
            return Err(format!("备份包含不支持的文件类型: {}", path.display()));
        }
    }
    Ok(result)
}

fn open_verified_database(path: &Path, label: &str) -> Result<Connection, String> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("无法只读打开{label}数据库: {error}"))?;
    let mut statement = connection
        .prepare("PRAGMA quick_check")
        .map_err(|error| format!("无法检查{label}数据库: {error}"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("无法检查{label}数据库: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取{label}数据库检查结果失败: {error}"))?;
    drop(statement);
    if rows.len() != 1 || !rows[0].eq_ignore_ascii_case("ok") {
        return Err(format!("{label}数据库完整性检查失败: {}", rows.join("; ")));
    }
    Ok(connection)
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .map_err(|error| format!("检查备份数据库表 {table} 失败: {error}"))
}

fn table_count(connection: &Connection, table: &str) -> Result<u64, String> {
    if !table_exists(connection, table)? {
        return Err(format!("备份数据库缺少必要表: {table}"));
    }
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .map_err(|error| format!("统计备份数据库表 {table} 失败: {error}"))
}

fn numeric_version(version: &str) -> Option<Vec<u64>> {
    let core = version
        .trim()
        .trim_start_matches('v')
        .split(['-', '+'])
        .next()?;
    let mut parts = core
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    parts.resize(3, 0);
    (!parts.is_empty()).then_some(parts)
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
        storage
            .execute("CREATE TABLE annotations (id TEXT PRIMARY KEY)", [])
            .unwrap();
        storage
            .execute(
                "INSERT INTO annotations VALUES (?1)",
                params!["annotation-1"],
            )
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

    fn refresh_manifest_entry(backup: &Path, relative: &str) {
        let manifest_path = backup.join("manifest.json");
        let mut manifest: BackupManifest =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        let path = backup.join(relative);
        let entry = manifest
            .files
            .iter_mut()
            .find(|file| file.path == relative)
            .unwrap();
        entry.size = fs::metadata(&path).unwrap().len();
        entry.sha256 = sha256_file(&path).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
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

    #[test]
    fn inspects_exported_backup_without_modifying_it() {
        let (source, storage, library) = fixture();
        let destination = TestDir::new("inspect");
        let exported =
            export_user_data_backup(&storage, &library, &source.0, &destination.0, "0.7.3")
                .unwrap();
        let backup = PathBuf::from(&exported.path);
        let manifest_before = fs::read(backup.join("manifest.json")).unwrap();

        let inspection = inspect_user_data_backup(&backup, "0.7.3").unwrap();

        assert_eq!(inspection.schema_version, BACKUP_SCHEMA_VERSION);
        assert_eq!(inspection.source_app_version, "0.7.3");
        assert_eq!(inspection.file_count, exported.file_count);
        assert_eq!(inspection.total_bytes, exported.total_bytes);
        assert_eq!(inspection.library_book_count, 1);
        assert_eq!(inspection.reading_progress_count, 1);
        assert_eq!(inspection.annotation_count, 1);
        assert_eq!(inspection.plugin_count, 1);
        assert_eq!(inspection.epub_file_count, 1);
        assert!(!inspection.newer_than_current_app);
        assert!(inspection.warnings.is_empty());
        assert_eq!(
            fs::read(backup.join("manifest.json")).unwrap(),
            manifest_before
        );
    }

    #[test]
    fn inspection_rejects_hash_drift_and_extra_payloads() {
        let (source, storage, library) = fixture();
        let destination = TestDir::new("tamper");
        let exported =
            export_user_data_backup(&storage, &library, &source.0, &destination.0, "0.7.3")
                .unwrap();
        let backup = PathBuf::from(&exported.path);
        fs::write(backup.join("library/objects/book.epub"), b"tampered").unwrap();
        let error = inspect_user_data_backup(&backup, "0.7.3").unwrap_err();
        assert!(error.contains("校验失败"));

        let (source, storage, library) = fixture();
        let destination = TestDir::new("extra");
        let exported =
            export_user_data_backup(&storage, &library, &source.0, &destination.0, "0.7.3")
                .unwrap();
        let backup = PathBuf::from(&exported.path);
        fs::write(backup.join("unexpected.txt"), b"not declared").unwrap();
        let error = inspect_user_data_backup(&backup, "0.7.3").unwrap_err();
        assert!(error.contains("清单外文件"));
    }

    #[test]
    fn inspection_rejects_unsafe_manifest_path_and_invalid_database() {
        let (source, storage, library) = fixture();
        let destination = TestDir::new("unsafe-path");
        let exported =
            export_user_data_backup(&storage, &library, &source.0, &destination.0, "0.7.3")
                .unwrap();
        let backup = PathBuf::from(&exported.path);
        let manifest_path = backup.join("manifest.json");
        let mut manifest: BackupManifest =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest.files[0].path = "../outside".into();
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let error = inspect_user_data_backup(&backup, "0.7.3").unwrap_err();
        assert!(error.contains("不安全路径"));

        let (source, storage, library) = fixture();
        let destination = TestDir::new("invalid-db");
        let exported =
            export_user_data_backup(&storage, &library, &source.0, &destination.0, "0.7.3")
                .unwrap();
        let backup = PathBuf::from(&exported.path);
        let reader_path = backup.join("reader.db");
        fs::write(&reader_path, b"not a sqlite database").unwrap();
        refresh_manifest_entry(&backup, "reader.db");
        let error = inspect_user_data_backup(&backup, "0.7.3").unwrap_err();
        assert!(error.contains("数据库"));
    }

    #[test]
    fn inspection_rejects_sqlite_without_required_reader_schema() {
        let (source, storage, library) = fixture();
        let destination = TestDir::new("missing-reader-schema");
        let exported =
            export_user_data_backup(&storage, &library, &source.0, &destination.0, "0.7.3")
                .unwrap();
        let backup = PathBuf::from(&exported.path);
        {
            let snapshot = Connection::open(backup.join("reader.db")).unwrap();
            snapshot.execute("DROP TABLE annotations", []).unwrap();
        }
        refresh_manifest_entry(&backup, "reader.db");

        let error = inspect_user_data_backup(&backup, "0.7.3").unwrap_err();
        assert!(error.contains("缺少必要表: annotations"));
    }

    #[test]
    fn inspection_warns_when_backup_version_is_newer() {
        let (source, storage, library) = fixture();
        let destination = TestDir::new("newer");
        let exported =
            export_user_data_backup(&storage, &library, &source.0, &destination.0, "9.0.0")
                .unwrap();

        let inspection = inspect_user_data_backup(Path::new(&exported.path), "0.7.3").unwrap();
        assert!(inspection.newer_than_current_app);
        assert_eq!(inspection.warnings.len(), 1);
        assert!(inspection.warnings[0].contains("较新的应用版本"));
    }
}
