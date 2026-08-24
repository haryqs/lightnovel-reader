//! 启动期用户数据恢复事务内核。
//!
//! 本模块只处理已关闭数据库连接后的文件系统事务，不负责 UI 二次确认、退出应用或创建新鲜回滚点。
//! 当前没有平台壳调用该入口；先用隔离目录与故障注入证明 staging、替换、复核和回滚语义。

use crate::backup::{inspect_user_data_backup, BackupManifest, UserDataBackupInspection};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const TRANSACTION_SCHEMA_VERSION: u32 = 1;
const REQUEST_FILE: &str = "request.json";
const STAGED_MARKER: &str = "staged.ok";
const DISPLACING_MARKER: &str = "displacing.ok";
const CURRENT_DISPLACED_MARKER: &str = "current-displaced.ok";
const ACTIVATING_MARKER: &str = "activating.ok";
const SOURCE_ACTIVATED_MARKER: &str = "source-activated.ok";
const STAGED_DIR: &str = "staged";
const DISPLACED_DIR: &str = "displaced";
const MANAGED_CURRENT_ENTRIES: [&str; 5] = [
    "reader.db",
    "reader.db-wal",
    "reader.db-shm",
    "library",
    "plugins",
];
const MANAGED_SOURCE_ENTRIES: [&str; 3] = ["reader.db", "library", "plugins"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RestoreTransactionRequest {
    schema_version: u32,
    app_data_path: String,
    source_backup_path: String,
    rollback_backup_path: String,
    source_manifest_sha256: String,
    rollback_manifest_sha256: String,
    original_entries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RestoreTransactionResult {
    pub schema_version: u32,
    pub completed_at: i64,
    pub source_backup_path: String,
    pub rollback_backup_path: String,
    pub restored_file_count: usize,
    pub restored_total_bytes: u64,
    pub resumed_incomplete_transaction: bool,
    pub restore_executed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestoreFaultPoint {
    None,
    AfterStaging,
    DuringDisplacement,
    AfterDisplacement,
    DuringActivation,
    AfterActivation,
    CorruptAfterActivation,
    BeforeRollback,
}

/// 在数据库连接全部关闭后执行低层恢复事务。
///
/// 当前平台壳没有调用该函数。未来接入必须先完成明确二次确认，并在持锁状态刷新外部回滚点后退出应用。
pub fn execute_user_data_restore_transaction_for_startup(
    app_data_dir: &Path,
    source_backup_dir: &Path,
    rollback_backup_dir: &Path,
    current_app_version: &str,
) -> Result<RestoreTransactionResult, String> {
    execute_with_fault(
        app_data_dir,
        source_backup_dir,
        rollback_backup_dir,
        current_app_version,
        RestoreFaultPoint::None,
    )
}

fn execute_with_fault(
    app_data_dir: &Path,
    source_backup_dir: &Path,
    rollback_backup_dir: &Path,
    current_app_version: &str,
    fault: RestoreFaultPoint,
) -> Result<RestoreTransactionResult, String> {
    let app_data_dir = validate_app_data_dir(app_data_dir)?;
    let transaction_root = transaction_root(&app_data_dir)?;
    let mut resumed = false;
    if transaction_root.exists() {
        resumed = true;
        match resume_existing_transaction(&transaction_root, &app_data_dir, fault)? {
            ResumeOutcome::Completed { request, manifest } => {
                return Ok(result_from_manifest(&request, &manifest, true));
            }
            ResumeOutcome::RestartFresh => {}
        }
    }

    let source = inspect_user_data_backup(source_backup_dir, current_app_version)?;
    let rollback = inspect_user_data_backup(rollback_backup_dir, current_app_version)?;
    if source.newer_than_current_app || rollback.newer_than_current_app {
        return Err("来源或回滚点来自较新的应用版本，启动期恢复已阻断".into());
    }
    let source_dir = PathBuf::from(&source.path)
        .canonicalize()
        .map_err(|error| format!("无法解析恢复来源目录: {error}"))?;
    let rollback_dir = PathBuf::from(&rollback.path)
        .canonicalize()
        .map_err(|error| format!("无法解析外部回滚点目录: {error}"))?;
    validate_external_backup_relationships(&app_data_dir, &source_dir, &rollback_dir)?;
    let source_manifest = read_manifest(&source_dir)?;
    let source_manifest_sha256 = sha256_file(&source_dir.join("manifest.json"))?;
    let rollback_manifest_sha256 = sha256_file(&rollback_dir.join("manifest.json"))?;

    let request = build_request(
        &app_data_dir,
        &source_dir,
        &rollback_dir,
        source_manifest_sha256,
        rollback_manifest_sha256,
    );

    fs::create_dir(&transaction_root).map_err(|error| format!("创建恢复事务目录失败: {error}"))?;
    write_json_create_new(&transaction_root.join(REQUEST_FILE), &request)?;
    let staged_dir = transaction_root.join(STAGED_DIR);
    copy_tree(&source_dir, &staged_dir)?;
    let staged = inspect_user_data_backup(&staged_dir, current_app_version)?;
    ensure_same_backup(&source, &staged, "staging")?;
    write_marker(&transaction_root, STAGED_MARKER)?;
    if fault == RestoreFaultPoint::AfterStaging {
        return Err("故障注入：staging 完成后中断".into());
    }

    write_marker(&transaction_root, DISPLACING_MARKER)?;
    displace_current_entries(
        &app_data_dir,
        &transaction_root.join(DISPLACED_DIR),
        &request.original_entries,
        fault,
    )?;
    write_marker(&transaction_root, CURRENT_DISPLACED_MARKER)?;
    if fault == RestoreFaultPoint::AfterDisplacement {
        return Err("故障注入：当前数据移出后中断".into());
    }

    write_marker(&transaction_root, ACTIVATING_MARKER)?;
    activate_staged_entries(&staged_dir, &app_data_dir, fault)?;
    write_marker(&transaction_root, SOURCE_ACTIVATED_MARKER)?;
    if fault == RestoreFaultPoint::AfterActivation {
        return Err("故障注入：来源激活后中断".into());
    }
    if fault == RestoreFaultPoint::CorruptAfterActivation {
        fs::write(app_data_dir.join("reader.db"), b"fault-injected-corruption")
            .map_err(|error| format!("故障注入写入失败: {error}"))?;
    }

    if let Err(error) = verify_active_payload(&app_data_dir, &source_manifest) {
        rollback_displaced_entries(
            &app_data_dir,
            &transaction_root.join(DISPLACED_DIR),
            &request.original_entries,
            fault,
        )?;
        remove_transaction_root(&transaction_root)?;
        return Err(format!("恢复后复核失败，已回滚当前数据: {error}"));
    }

    remove_transaction_root(&transaction_root)?;
    Ok(result_from(&source, &rollback, resumed))
}

enum ResumeOutcome {
    Completed {
        request: Box<RestoreTransactionRequest>,
        manifest: Box<BackupManifest>,
    },
    RestartFresh,
}

fn resume_existing_transaction(
    transaction_root: &Path,
    app_data_dir: &Path,
    fault: RestoreFaultPoint,
) -> Result<ResumeOutcome, String> {
    let metadata = fs::symlink_metadata(transaction_root)
        .map_err(|error| format!("读取遗留恢复事务失败: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("遗留恢复事务路径不是普通目录，拒绝继续".into());
    }
    let mutation_started = mutation_started(transaction_root);
    let request = match read_existing_request(transaction_root, app_data_dir) {
        Ok(request) => request,
        Err(_error) if !mutation_started => {
            remove_transaction_root(transaction_root)?;
            return Ok(ResumeOutcome::RestartFresh);
        }
        Err(error) => return Err(error),
    };

    if transaction_root.join(SOURCE_ACTIVATED_MARKER).is_file() {
        let staged_manifest_path = transaction_root.join(STAGED_DIR).join("manifest.json");
        let staged_manifest = read_manifest_file(&staged_manifest_path).and_then(|manifest| {
            if sha256_file(&staged_manifest_path)? != request.source_manifest_sha256 {
                return Err("遗留 staging manifest 摘要与事务请求不一致".into());
            }
            Ok(manifest)
        });
        match staged_manifest.and_then(|manifest| {
            verify_active_payload(app_data_dir, &manifest)?;
            Ok(manifest)
        }) {
            Ok(manifest) => {
                remove_transaction_root(transaction_root)?;
                return Ok(ResumeOutcome::Completed {
                    request: Box::new(request),
                    manifest: Box::new(manifest),
                });
            }
            Err(error) => {
                rollback_displaced_entries(
                    app_data_dir,
                    &transaction_root.join(DISPLACED_DIR),
                    &request.original_entries,
                    fault,
                )?;
                remove_transaction_root(transaction_root)?;
                return Err(format!("遗留恢复结果复核失败，已回滚: {error}"));
            }
        }
    }

    if transaction_root.join(DISPLACING_MARKER).is_file()
        || transaction_root.join(CURRENT_DISPLACED_MARKER).is_file()
        || transaction_root.join(ACTIVATING_MARKER).is_file()
    {
        rollback_displaced_entries(
            app_data_dir,
            &transaction_root.join(DISPLACED_DIR),
            &request.original_entries,
            fault,
        )?;
    }
    remove_transaction_root(transaction_root)?;
    Ok(ResumeOutcome::RestartFresh)
}

fn validate_app_data_dir(path: &Path) -> Result<PathBuf, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("读取应用数据目录失败: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("应用数据路径必须是普通目录，不能是符号链接".into());
    }
    let path = path
        .canonicalize()
        .map_err(|error| format!("无法解析应用数据目录: {error}"))?;
    for name in MANAGED_CURRENT_ENTRIES {
        let entry = path.join(name);
        if let Ok(metadata) = fs::symlink_metadata(&entry) {
            if metadata.file_type().is_symlink() {
                return Err(format!("应用数据包含不安全符号链接: {name}"));
            }
        }
    }
    Ok(path)
}

fn validate_external_backup_relationships(
    app_data_dir: &Path,
    source_dir: &Path,
    rollback_dir: &Path,
) -> Result<(), String> {
    if paths_equal(source_dir, rollback_dir) {
        return Err("恢复来源与外部回滚点不能是同一目录".into());
    }
    if path_is_within(source_dir, app_data_dir)
        || path_is_within(rollback_dir, app_data_dir)
        || path_is_within(app_data_dir, source_dir)
        || path_is_within(app_data_dir, rollback_dir)
    {
        return Err("恢复来源、外部回滚点与应用数据目录不能互相包含".into());
    }
    Ok(())
}

fn build_request(
    app_data_dir: &Path,
    source_dir: &Path,
    rollback_dir: &Path,
    source_manifest_sha256: String,
    rollback_manifest_sha256: String,
) -> RestoreTransactionRequest {
    let original_entries = MANAGED_CURRENT_ENTRIES
        .iter()
        .filter(|name| app_data_dir.join(name).exists())
        .map(|name| (*name).to_string())
        .collect();
    RestoreTransactionRequest {
        schema_version: TRANSACTION_SCHEMA_VERSION,
        app_data_path: display_path(app_data_dir),
        source_backup_path: display_path(source_dir),
        rollback_backup_path: display_path(rollback_dir),
        source_manifest_sha256,
        rollback_manifest_sha256,
        original_entries,
    }
}

fn validate_existing_request(
    request: &RestoreTransactionRequest,
    app_data_dir: &Path,
) -> Result<(), String> {
    if request.schema_version != TRANSACTION_SCHEMA_VERSION
        || request.app_data_path != display_path(app_data_dir)
        || request.source_backup_path.is_empty()
        || request.rollback_backup_path.is_empty()
        || !is_sha256(&request.source_manifest_sha256)
        || !is_sha256(&request.rollback_manifest_sha256)
    {
        return Err("遗留恢复事务的 schema、路径或清单摘要无效".into());
    }

    let allowed = MANAGED_CURRENT_ENTRIES.into_iter().collect::<BTreeSet<_>>();
    let original = request
        .original_entries
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if original.len() != request.original_entries.len()
        || original.iter().any(|name| !allowed.contains(name))
    {
        return Err("遗留恢复事务包含非法或重复的原始数据项".into());
    }
    Ok(())
}

fn read_existing_request(
    transaction_root: &Path,
    app_data_dir: &Path,
) -> Result<RestoreTransactionRequest, String> {
    let path = transaction_root.join(REQUEST_FILE);
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("读取遗留恢复事务请求失败: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("遗留恢复事务请求不是普通文件".into());
    }
    let request = serde_json::from_slice(
        &fs::read(&path).map_err(|error| format!("读取遗留恢复事务请求失败: {error}"))?,
    )
    .map_err(|error| format!("解析遗留恢复事务请求失败: {error}"))?;
    validate_existing_request(&request, app_data_dir)?;
    Ok(request)
}

fn mutation_started(transaction_root: &Path) -> bool {
    [
        DISPLACING_MARKER,
        CURRENT_DISPLACED_MARKER,
        ACTIVATING_MARKER,
        SOURCE_ACTIVATED_MARKER,
    ]
    .iter()
    .any(|name| transaction_root.join(name).exists())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn result_from(
    source: &UserDataBackupInspection,
    rollback: &UserDataBackupInspection,
    resumed: bool,
) -> RestoreTransactionResult {
    RestoreTransactionResult {
        schema_version: TRANSACTION_SCHEMA_VERSION,
        completed_at: now_ms(),
        source_backup_path: source.path.clone(),
        rollback_backup_path: rollback.path.clone(),
        restored_file_count: source.file_count,
        restored_total_bytes: source.total_bytes,
        resumed_incomplete_transaction: resumed,
        restore_executed: true,
    }
}

fn result_from_manifest(
    request: &RestoreTransactionRequest,
    manifest: &BackupManifest,
    resumed: bool,
) -> RestoreTransactionResult {
    RestoreTransactionResult {
        schema_version: TRANSACTION_SCHEMA_VERSION,
        completed_at: now_ms(),
        source_backup_path: request.source_backup_path.clone(),
        rollback_backup_path: request.rollback_backup_path.clone(),
        restored_file_count: manifest.files.len(),
        restored_total_bytes: manifest.files.iter().map(|file| file.size).sum(),
        resumed_incomplete_transaction: resumed,
        restore_executed: true,
    }
}

fn transaction_root(app_data_dir: &Path) -> Result<PathBuf, String> {
    let parent = app_data_dir
        .parent()
        .ok_or_else(|| "应用数据目录缺少父目录，无法创建同卷恢复事务".to_string())?;
    let name = app_data_dir
        .file_name()
        .ok_or_else(|| "应用数据目录缺少目录名".to_string())?
        .to_string_lossy();
    Ok(parent.join(format!(".{name}.restore-transaction-v1")))
}

fn displace_current_entries(
    app_data_dir: &Path,
    displaced_dir: &Path,
    original_entries: &[String],
    fault: RestoreFaultPoint,
) -> Result<(), String> {
    fs::create_dir(displaced_dir).map_err(|error| format!("创建当前数据暂存目录失败: {error}"))?;
    for (index, name) in original_entries.iter().enumerate() {
        fs::rename(app_data_dir.join(name), displaced_dir.join(name))
            .map_err(|error| format!("暂存当前数据失败 {name}: {error}"))?;
        if fault == RestoreFaultPoint::DuringDisplacement && index == 0 {
            return Err("故障注入：当前数据移出过程中中断".into());
        }
    }
    Ok(())
}

fn activate_staged_entries(
    staged_dir: &Path,
    app_data_dir: &Path,
    fault: RestoreFaultPoint,
) -> Result<(), String> {
    let mut moved = 0_usize;
    for name in MANAGED_SOURCE_ENTRIES {
        let source = staged_dir.join(name);
        if !source.exists() {
            continue;
        }
        fs::rename(&source, app_data_dir.join(name))
            .map_err(|error| format!("激活恢复数据失败 {name}: {error}"))?;
        moved += 1;
        if fault == RestoreFaultPoint::DuringActivation && moved == 1 {
            return Err("故障注入：来源激活过程中中断".into());
        }
    }
    Ok(())
}

fn rollback_displaced_entries(
    app_data_dir: &Path,
    displaced_dir: &Path,
    original_entries: &[String],
    fault: RestoreFaultPoint,
) -> Result<(), String> {
    if fault == RestoreFaultPoint::BeforeRollback {
        return Err("故障注入：回滚开始前中断；事务保留供下次启动恢复".into());
    }
    if displaced_dir.exists() {
        let metadata = fs::symlink_metadata(displaced_dir)
            .map_err(|error| format!("读取当前数据暂存目录失败: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("当前数据暂存路径不是普通目录，拒绝回滚".into());
        }
    }
    let original = original_entries.iter().cloned().collect::<BTreeSet<_>>();
    for name in MANAGED_CURRENT_ENTRIES {
        let current = app_data_dir.join(name);
        let displaced = displaced_dir.join(name);
        if original.contains(name) {
            if displaced.exists() {
                remove_path_if_exists(&current)?;
                fs::rename(&displaced, &current)
                    .map_err(|error| format!("回滚当前数据失败 {name}: {error}"))?;
            }
        } else {
            remove_path_if_exists(&current)?;
        }
    }
    Ok(())
}

fn verify_active_payload(app_data_dir: &Path, manifest: &BackupManifest) -> Result<(), String> {
    let declared = manifest
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let actual = collect_active_payload_files(app_data_dir)?;
    if declared != actual {
        return Err("激活后的文件集合与来源清单不一致".into());
    }
    for file in &manifest.files {
        let path = app_data_dir.join(Path::new(&file.path));
        let metadata = fs::metadata(&path)
            .map_err(|error| format!("读取激活文件失败 {}: {error}", file.path))?;
        if metadata.len() != file.size || sha256_file(&path)? != file.sha256 {
            return Err(format!("激活文件校验失败: {}", file.path));
        }
    }
    quick_check_database(&app_data_dir.join("reader.db"), "阅读数据")?;
    quick_check_database(&app_data_dir.join("library/library.sqlite"), "书库")?;
    Ok(())
}

fn collect_active_payload_files(app_data_dir: &Path) -> Result<BTreeSet<String>, String> {
    let mut files = BTreeSet::new();
    let reader = app_data_dir.join("reader.db");
    if reader.is_file() {
        files.insert("reader.db".into());
    }
    for name in ["library", "plugins"] {
        let directory = app_data_dir.join(name);
        if directory.is_dir() {
            collect_files(app_data_dir, &directory, &mut files)?;
        }
    }
    Ok(files)
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut BTreeSet<String>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("读取恢复目录失败 {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("枚举恢复目录失败 {}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("读取恢复条目失败 {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("恢复数据包含符号链接: {}", path.display()));
        }
        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else if metadata.is_file() {
            files.insert(portable_relative_path(root, &path)?);
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| format!("读取 staging 来源失败 {}: {error}", source.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("staging 来源包含符号链接: {}", source.display()));
    }
    fs::create_dir(target)
        .map_err(|error| format!("创建 staging 目录失败 {}: {error}", target.display()))?;
    let mut entries = fs::read_dir(source)
        .map_err(|error| format!("读取 staging 来源失败 {}: {error}", source.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("枚举 staging 来源失败 {}: {error}", source.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path)
            .map_err(|error| format!("读取 staging 条目失败 {}: {error}", source_path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "staging 来源包含符号链接: {}",
                source_path.display()
            ));
        }
        if metadata.is_dir() {
            copy_tree(&source_path, &target_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &target_path).map_err(|error| {
                format!(
                    "复制 staging 文件失败 {} -> {}: {error}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn ensure_same_backup(
    source: &UserDataBackupInspection,
    staged: &UserDataBackupInspection,
    label: &str,
) -> Result<(), String> {
    if source.schema_version != staged.schema_version
        || source.source_app_version != staged.source_app_version
        || source.created_at != staged.created_at
        || source.file_count != staged.file_count
        || source.total_bytes != staged.total_bytes
        || source.library_book_count != staged.library_book_count
        || source.reading_progress_count != staged.reading_progress_count
        || source.annotation_count != staged.annotation_count
        || source.plugin_count != staged.plugin_count
        || source.epub_file_count != staged.epub_file_count
    {
        return Err(format!("{label} 内容摘要与恢复来源不一致"));
    }
    Ok(())
}

fn quick_check_database(path: &Path, label: &str) -> Result<(), String> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("只读打开激活后的{label}数据库失败: {error}"))?;
    let mut statement = connection
        .prepare("PRAGMA quick_check")
        .map_err(|error| format!("准备激活后的{label}数据库检查失败: {error}"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("执行激活后的{label}数据库检查失败: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取激活后的{label}数据库检查结果失败: {error}"))?;
    if rows.len() != 1 || rows[0] != "ok" {
        return Err(format!("激活后的{label}数据库完整性检查失败"));
    }
    Ok(())
}

fn read_manifest(backup_dir: &Path) -> Result<BackupManifest, String> {
    read_manifest_file(&backup_dir.join("manifest.json"))
}

fn read_manifest_file(path: &Path) -> Result<BackupManifest, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("读取恢复来源 manifest 失败: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("恢复来源 manifest 必须是普通文件".into());
    }
    serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("读取恢复来源 manifest 失败: {error}"))?,
    )
    .map_err(|error| format!("解析恢复来源 manifest 失败: {error}"))
}

fn write_json_create_new<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let mut json = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("序列化恢复事务请求失败: {error}"))?;
    json.push(b'\n');
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("创建恢复事务请求失败: {error}"))?;
    file.write_all(&json)
        .map_err(|error| format!("写入恢复事务请求失败: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("同步恢复事务请求失败: {error}"))
}

fn write_marker(transaction_root: &Path, name: &str) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(transaction_root.join(name))
        .map_err(|error| format!("创建恢复阶段标记 {name} 失败: {error}"))?;
    file.write_all(b"ok\n")
        .map_err(|error| format!("写入恢复阶段标记 {name} 失败: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("同步恢复阶段标记 {name} 失败: {error}"))
}

fn remove_transaction_root(path: &Path) -> Result<(), String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if !name.starts_with('.') || !name.ends_with(".restore-transaction-v1") {
        return Err("拒绝清理名称异常的恢复事务目录".into());
    }
    fs::remove_dir_all(path).map_err(|error| format!("清理恢复事务目录失败: {error}"))
}

fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "读取待清理恢复路径失败 {}: {error}",
                path.display()
            ))
        }
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("清理恢复目录失败 {}: {error}", path.display()))
    } else {
        fs::remove_file(path)
            .map_err(|error| format!("清理恢复文件失败 {}: {error}", path.display()))
    }
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("读取摘要文件失败 {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("计算文件摘要失败 {}: {error}", path.display()))?;
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
    Ok(path
        .strip_prefix(root)
        .map_err(|error| format!("恢复文件不在应用数据目录内: {error}"))?
        .to_string_lossy()
        .replace('\\', "/"))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
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

fn paths_equal(left: &Path, right: &Path) -> bool {
    path_is_within(left, right) && path_is_within(right, left)
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
    use crate::backup::export_user_data_backup;
    use rusqlite::params;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "lightnovel-reader-restore-{label}-{}-{nonce}",
                std::process::id()
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

    struct Fixture {
        _root: TestDir,
        app_data: PathBuf,
        source_backup: PathBuf,
        rollback_backup: PathBuf,
    }

    fn fixture() -> Fixture {
        let root = TestDir::new("transaction");
        let app_data = root.0.join("app-data");
        fs::create_dir_all(app_data.join("library/objects")).unwrap();
        fs::create_dir_all(app_data.join("plugins/sources/demo")).unwrap();
        fs::create_dir_all(app_data.join("cache/parsed")).unwrap();
        fs::write(app_data.join("library/objects/source.epub"), b"source").unwrap();
        fs::write(
            app_data.join("plugins/sources/demo/manifest.json"),
            b"source-plugin",
        )
        .unwrap();
        fs::write(app_data.join("cache/parsed/stale"), b"cache").unwrap();
        fs::write(app_data.join("sync.json"), b"secret-sync-token").unwrap();

        let storage = Connection::open(app_data.join("reader.db")).unwrap();
        storage
            .execute("CREATE TABLE reading_state (book_id TEXT PRIMARY KEY)", [])
            .unwrap();
        storage
            .execute(
                "INSERT INTO reading_state VALUES (?1)",
                params!["source-book"],
            )
            .unwrap();
        storage
            .execute("CREATE TABLE annotations (id TEXT PRIMARY KEY)", [])
            .unwrap();
        storage
            .execute(
                "INSERT INTO annotations VALUES (?1)",
                params!["source-note"],
            )
            .unwrap();
        let library = Connection::open(app_data.join("library/library.sqlite")).unwrap();
        library
            .execute("CREATE TABLE books (id TEXT PRIMARY KEY)", [])
            .unwrap();
        library
            .execute("INSERT INTO books VALUES (?1)", params!["source-book"])
            .unwrap();
        let backups = root.0.join("backups");
        fs::create_dir(&backups).unwrap();
        let source =
            export_user_data_backup(&storage, &library, &app_data, &backups, "0.7.3").unwrap();

        storage.execute("DELETE FROM reading_state", []).unwrap();
        storage
            .execute(
                "INSERT INTO reading_state VALUES (?1)",
                params!["current-book"],
            )
            .unwrap();
        library.execute("DELETE FROM books", []).unwrap();
        library
            .execute("INSERT INTO books VALUES (?1)", params!["current-book"])
            .unwrap();
        fs::remove_file(app_data.join("library/objects/source.epub")).unwrap();
        fs::write(app_data.join("library/objects/current.epub"), b"current").unwrap();
        fs::write(
            app_data.join("plugins/sources/demo/manifest.json"),
            b"current-plugin",
        )
        .unwrap();
        let rollback =
            export_user_data_backup(&storage, &library, &app_data, &backups, "0.7.3").unwrap();
        drop(library);
        drop(storage);

        Fixture {
            _root: root,
            app_data,
            source_backup: PathBuf::from(source.path),
            rollback_backup: PathBuf::from(rollback.path),
        }
    }

    fn reading_book(app_data: &Path) -> String {
        Connection::open(app_data.join("reader.db"))
            .unwrap()
            .query_row("SELECT book_id FROM reading_state", [], |row| row.get(0))
            .unwrap()
    }

    fn assert_current(fixture: &Fixture) {
        assert_eq!(reading_book(&fixture.app_data), "current-book");
        assert!(fixture
            .app_data
            .join("library/objects/current.epub")
            .is_file());
        assert!(!fixture
            .app_data
            .join("library/objects/source.epub")
            .exists());
        assert_eq!(
            fs::read(fixture.app_data.join("sync.json")).unwrap(),
            b"secret-sync-token"
        );
    }

    fn assert_source(fixture: &Fixture) {
        assert_eq!(reading_book(&fixture.app_data), "source-book");
        assert!(fixture
            .app_data
            .join("library/objects/source.epub")
            .is_file());
        assert!(!fixture
            .app_data
            .join("library/objects/current.epub")
            .exists());
        assert_eq!(
            fs::read(fixture.app_data.join("sync.json")).unwrap(),
            b"secret-sync-token"
        );
    }

    fn execute(
        fixture: &Fixture,
        fault: RestoreFaultPoint,
    ) -> Result<RestoreTransactionResult, String> {
        execute_with_fault(
            &fixture.app_data,
            &fixture.source_backup,
            &fixture.rollback_backup,
            "0.7.3",
            fault,
        )
    }

    #[test]
    fn restores_managed_data_and_preserves_excluded_state() {
        let fixture = fixture();
        assert_current(&fixture);

        let result = execute_user_data_restore_transaction_for_startup(
            &fixture.app_data,
            &fixture.source_backup,
            &fixture.rollback_backup,
            "0.7.3",
        )
        .unwrap();

        assert!(result.restore_executed);
        assert!(!result.resumed_incomplete_transaction);
        assert_source(&fixture);
        assert!(fixture.app_data.join("cache/parsed/stale").is_file());
        assert!(!transaction_root(&fixture.app_data).unwrap().exists());
    }

    #[test]
    fn resumes_after_staging_and_after_source_activation() {
        let staged_fixture = fixture();
        assert!(execute(&staged_fixture, RestoreFaultPoint::AfterStaging).is_err());
        assert_current(&staged_fixture);
        let result = execute(&staged_fixture, RestoreFaultPoint::None).unwrap();
        assert!(result.resumed_incomplete_transaction);
        assert_source(&staged_fixture);

        let activated_fixture = fixture();
        assert!(execute(&activated_fixture, RestoreFaultPoint::AfterActivation).is_err());
        assert_source(&activated_fixture);
        let result = execute(&activated_fixture, RestoreFaultPoint::None).unwrap();
        assert!(result.resumed_incomplete_transaction);
        assert_source(&activated_fixture);
        assert!(!transaction_root(&activated_fixture.app_data)
            .unwrap()
            .exists());
    }

    #[test]
    fn rolls_back_partial_displacement_and_activation_before_retrying() {
        for fault in [
            RestoreFaultPoint::DuringDisplacement,
            RestoreFaultPoint::AfterDisplacement,
            RestoreFaultPoint::DuringActivation,
        ] {
            let fixture = fixture();
            assert!(execute(&fixture, fault).is_err());
            let result = execute(&fixture, RestoreFaultPoint::None).unwrap();
            assert!(result.resumed_incomplete_transaction);
            assert_source(&fixture);
        }
    }

    #[test]
    fn verification_failure_rolls_back_and_rollback_failure_is_retryable() {
        let corrupt_fixture = fixture();
        let error =
            execute(&corrupt_fixture, RestoreFaultPoint::CorruptAfterActivation).unwrap_err();
        assert!(error.contains("已回滚"));
        assert_current(&corrupt_fixture);

        let retry_fixture = fixture();
        assert!(execute(&retry_fixture, RestoreFaultPoint::DuringActivation).is_err());
        let error = execute(&retry_fixture, RestoreFaultPoint::BeforeRollback).unwrap_err();
        assert!(error.contains("事务保留"));
        let result = execute(&retry_fixture, RestoreFaultPoint::None).unwrap();
        assert!(result.resumed_incomplete_transaction);
        assert_source(&retry_fixture);
    }

    #[test]
    fn recovers_without_external_backups_after_local_mutation_started() {
        let rollback_fixture = fixture();
        assert!(execute(&rollback_fixture, RestoreFaultPoint::DuringActivation).is_err());
        fs::remove_dir_all(&rollback_fixture.source_backup).unwrap();
        fs::remove_dir_all(&rollback_fixture.rollback_backup).unwrap();
        let error = execute(&rollback_fixture, RestoreFaultPoint::None).unwrap_err();
        assert!(error.contains("备份目录") || error.contains("读取"));
        assert_current(&rollback_fixture);
        assert!(!transaction_root(&rollback_fixture.app_data)
            .unwrap()
            .exists());

        let completed_fixture = fixture();
        assert!(execute(&completed_fixture, RestoreFaultPoint::AfterActivation).is_err());
        fs::remove_dir_all(&completed_fixture.source_backup).unwrap();
        fs::remove_dir_all(&completed_fixture.rollback_backup).unwrap();
        let result = execute(&completed_fixture, RestoreFaultPoint::None).unwrap();
        assert!(result.resumed_incomplete_transaction);
        assert_source(&completed_fixture);
        assert!(!transaction_root(&completed_fixture.app_data)
            .unwrap()
            .exists());
    }

    #[test]
    fn discards_pre_mutation_partial_initialization_before_retrying() {
        let fixture = fixture();
        fs::create_dir(transaction_root(&fixture.app_data).unwrap()).unwrap();

        let result = execute(&fixture, RestoreFaultPoint::None).unwrap();

        assert!(result.resumed_incomplete_transaction);
        assert_source(&fixture);
        assert!(!transaction_root(&fixture.app_data).unwrap().exists());
    }
}
