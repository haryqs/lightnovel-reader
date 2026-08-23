//! 用户数据备份：生成可校验、可迁移的独立目录。
//!
//! 两个 SQLite 数据库必须通过现有连接创建一致性快照，不能复制正在使用的数据库文件；
//! Tauri 等平台壳只负责选择目标目录和持有连接锁。

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const BACKUP_SCHEMA_VERSION: u32 = 1;
const BACKUP_PREFIX: &str = "lightnovel-reader-backup-v1";
const RESTORE_PREPARATION_PREFIX: &str = "lightnovel-reader-restore-preparation-v1";
const MAX_MANIFEST_BYTES: u64 = 8 * 1024 * 1024;
const MAX_RESTORE_PREPARATION_BYTES: u64 = 8 * 1024 * 1024;
const MAX_BACKUP_FILES: usize = 200_000;
const MIN_RESTORE_SAFETY_MARGIN_BYTES: u64 = 64 * 1024 * 1024;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserDataRestorePlan {
    pub backup: UserDataBackupInspection,
    pub current_library_book_count: u64,
    pub current_reading_progress_count: u64,
    pub current_annotation_count: u64,
    pub current_plugin_count: usize,
    pub current_epub_file_count: usize,
    pub rollback_estimated_bytes: u64,
    pub replacement_file_count: usize,
    pub requires_restart: bool,
    pub requires_pre_restore_backup: bool,
    pub version_compatible: bool,
    pub blocked_reasons: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserDataRestorePreparation {
    pub schema_version: u32,
    pub prepared_at: i64,
    pub plan: UserDataRestorePlan,
    pub rollback_backup: UserDataBackupInspection,
    pub source_manifest_sha256: String,
    pub rollback_manifest_sha256: String,
    pub receipt_path: String,
    pub requires_restart: bool,
    pub restore_executed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserDataRestorePreflight {
    pub schema_version: u32,
    pub checked_at: i64,
    pub receipt_path: String,
    pub source_backup: UserDataBackupInspection,
    pub rollback_backup: UserDataBackupInspection,
    pub source_manifest_sha256: String,
    pub rollback_manifest_sha256: String,
    pub target_available_bytes: u64,
    pub required_staging_bytes: u64,
    pub safety_margin_bytes: u64,
    pub required_total_bytes: u64,
    pub preflight_passed: bool,
    pub requires_fresh_rollback_at_execution: bool,
    pub restore_authorized: bool,
    pub restore_executed: bool,
    pub blocked_reasons: Vec<String>,
    pub warnings: Vec<String>,
}

/// 只读生成恢复事务计划。该计划比较当前数据与已验证备份，不执行复制或覆盖。
pub fn plan_user_data_restore(
    storage_db: &Connection,
    library_db: &Connection,
    app_data_dir: &Path,
    backup_dir: &Path,
    current_app_version: &str,
) -> Result<UserDataRestorePlan, String> {
    let backup_metadata =
        fs::symlink_metadata(backup_dir).map_err(|error| format!("读取备份目录失败: {error}"))?;
    if backup_metadata.file_type().is_symlink() {
        return Err("恢复来源不能是符号链接".into());
    }
    let app_data_dir = app_data_dir
        .canonicalize()
        .map_err(|error| format!("无法解析应用数据目录: {error}"))?;
    let backup_dir = backup_dir
        .canonicalize()
        .map_err(|error| format!("无法解析备份目录: {error}"))?;
    if path_is_within(&backup_dir, &app_data_dir) {
        return Err("恢复来源不能位于应用数据目录内部".into());
    }
    let backup = inspect_user_data_backup(&backup_dir, current_app_version)?;

    let current_library_book_count = if table_exists(library_db, "edition")? {
        table_count(library_db, "edition")?
    } else {
        table_count(library_db, "books")?
    };
    let current_reading_progress_count = table_count(storage_db, "reading_state")?;
    let current_annotation_count = table_count(storage_db, "annotations")?;
    let current_files = collect_current_backup_payload_files(&app_data_dir)?;
    let current_plugin_count = plugin_count_from_paths(&current_files);
    let current_epub_file_count = epub_count_from_paths(&current_files);
    let file_bytes = current_files.iter().try_fold(0_u64, |total, relative| {
        let size = fs::metadata(app_data_dir.join(Path::new(relative)))
            .map_err(|error| format!("读取当前数据文件信息失败 {relative}: {error}"))?
            .len();
        total
            .checked_add(size)
            .ok_or_else(|| "当前数据文件总大小溢出".to_string())
    })?;
    let storage_snapshot_bytes = sqlite_snapshot_estimate(storage_db)?;
    let library_snapshot_bytes = sqlite_snapshot_estimate(library_db)?;
    let rollback_estimated_bytes = file_bytes
        .checked_add(storage_snapshot_bytes)
        .and_then(|total| total.checked_add(library_snapshot_bytes))
        .ok_or_else(|| "回滚点预计大小溢出".to_string())?;

    let mut blocked_reasons = Vec::new();
    if backup.newer_than_current_app {
        blocked_reasons.push(format!(
            "备份来自较新的应用版本 v{}，当前 v{} 不允许进入恢复事务",
            backup.source_app_version, current_app_version
        ));
    }
    let warnings = vec![
        "恢复将整体替换当前阅读进度、标注、书库资产与插件数据，不执行自动合并".into(),
        "正式恢复前必须在外部目录创建当前数据回滚备份，并在关闭活动数据库连接后重启应用".into(),
    ];

    Ok(UserDataRestorePlan {
        replacement_file_count: backup.file_count,
        version_compatible: blocked_reasons.is_empty(),
        backup,
        current_library_book_count,
        current_reading_progress_count,
        current_annotation_count,
        current_plugin_count,
        current_epub_file_count,
        rollback_estimated_bytes,
        requires_restart: true,
        requires_pre_restore_backup: true,
        blocked_reasons,
        warnings,
    })
}

/// 创建并复核当前数据的外部回滚点，再写入可审计的准备凭据。
/// 此函数不会关闭数据库、替换应用数据或安排下次启动恢复。
pub fn prepare_user_data_restore(
    storage_db: &Connection,
    library_db: &Connection,
    app_data_dir: &Path,
    source_backup_dir: &Path,
    rollback_parent: &Path,
    current_app_version: &str,
) -> Result<UserDataRestorePreparation, String> {
    let initial_plan = plan_user_data_restore(
        storage_db,
        library_db,
        app_data_dir,
        source_backup_dir,
        current_app_version,
    )?;
    ensure_restore_plan_is_compatible(&initial_plan)?;

    let source_backup_dir = PathBuf::from(&initial_plan.backup.path)
        .canonicalize()
        .map_err(|error| format!("无法重新解析恢复来源目录: {error}"))?;
    let rollback_metadata = fs::symlink_metadata(rollback_parent)
        .map_err(|error| format!("读取回滚点目标目录失败: {error}"))?;
    if rollback_metadata.file_type().is_symlink() || !rollback_metadata.is_dir() {
        return Err("回滚点目标必须是普通文件夹，不能是符号链接".into());
    }
    let rollback_parent = rollback_parent
        .canonicalize()
        .map_err(|error| format!("无法解析回滚点目标目录: {error}"))?;
    if path_is_within(&rollback_parent, &source_backup_dir) {
        return Err("回滚点目标不能位于恢复来源目录内部".into());
    }

    let rollback_result = export_user_data_backup(
        storage_db,
        library_db,
        app_data_dir,
        &rollback_parent,
        current_app_version,
    )?;
    let rollback_dir = PathBuf::from(&rollback_result.path);
    let prepared = (|| {
        let rollback_backup = inspect_user_data_backup(&rollback_dir, current_app_version)?;
        if rollback_backup.newer_than_current_app {
            return Err("新建回滚点的版本校验异常".into());
        }

        // 回滚点创建可能耗时；写凭据前再次完整校验恢复来源并刷新事务计划。
        let plan = plan_user_data_restore(
            storage_db,
            library_db,
            app_data_dir,
            &source_backup_dir,
            current_app_version,
        )?;
        ensure_restore_plan_is_compatible(&plan)?;

        let prepared_at = now_ms();
        let (partial_receipt, final_receipt) =
            unique_restore_preparation_paths(&rollback_parent, prepared_at);
        let preparation = UserDataRestorePreparation {
            schema_version: 1,
            prepared_at,
            source_manifest_sha256: sha256_file(&source_backup_dir.join("manifest.json"))?,
            rollback_manifest_sha256: sha256_file(&rollback_dir.join("manifest.json"))?,
            receipt_path: display_path(&final_receipt),
            plan,
            rollback_backup,
            requires_restart: true,
            restore_executed: false,
        };
        write_restore_preparation_receipt(&partial_receipt, &final_receipt, &preparation)?;
        Ok(preparation)
    })();

    match prepared {
        Ok(preparation) => Ok(preparation),
        Err(error) => {
            let _ = fs::remove_dir_all(&rollback_dir);
            Err(error)
        }
    }
}

/// 只读复核恢复准备凭据、来源/回滚点和目标卷空间。
/// 通过预检不代表用户已授权恢复，也不会写入或替换应用数据。
pub fn preflight_user_data_restore(
    receipt_path: &Path,
    app_data_dir: &Path,
    current_app_version: &str,
) -> Result<UserDataRestorePreflight, String> {
    let available_bytes = fs2::available_space(app_data_dir)
        .map_err(|error| format!("读取应用数据所在磁盘的可用空间失败: {error}"))?;
    preflight_user_data_restore_with_available_bytes(
        receipt_path,
        app_data_dir,
        current_app_version,
        available_bytes,
    )
}

fn preflight_user_data_restore_with_available_bytes(
    receipt_path: &Path,
    app_data_dir: &Path,
    current_app_version: &str,
    target_available_bytes: u64,
) -> Result<UserDataRestorePreflight, String> {
    if current_app_version.trim().is_empty() {
        return Err("当前应用版本不能为空".into());
    }
    let receipt_metadata = fs::symlink_metadata(receipt_path)
        .map_err(|error| format!("读取恢复准备凭据失败: {error}"))?;
    if receipt_metadata.file_type().is_symlink() || !receipt_metadata.is_file() {
        return Err("恢复准备凭据必须是普通 JSON 文件，不能是符号链接".into());
    }
    if receipt_metadata.len() > MAX_RESTORE_PREPARATION_BYTES {
        return Err(format!(
            "恢复准备凭据超过 {} MiB 安全上限",
            MAX_RESTORE_PREPARATION_BYTES / 1024 / 1024
        ));
    }
    let receipt_path = receipt_path
        .canonicalize()
        .map_err(|error| format!("无法解析恢复准备凭据路径: {error}"))?;
    let app_data_dir = app_data_dir
        .canonicalize()
        .map_err(|error| format!("无法解析应用数据目录: {error}"))?;
    if path_is_within(&receipt_path, &app_data_dir) {
        return Err("恢复准备凭据不能位于应用数据目录内部".into());
    }

    let preparation: UserDataRestorePreparation = serde_json::from_slice(
        &fs::read(&receipt_path).map_err(|error| format!("读取恢复准备凭据失败: {error}"))?,
    )
    .map_err(|error| format!("解析恢复准备凭据失败: {error}"))?;
    validate_restore_preparation_shape(&preparation)?;

    let recorded_receipt_path = PathBuf::from(&preparation.receipt_path)
        .canonicalize()
        .map_err(|error| format!("无法解析凭据记录的自身路径: {error}"))?;
    if !paths_equal(&receipt_path, &recorded_receipt_path) {
        return Err("恢复准备凭据路径与其记录的 receiptPath 不一致".into());
    }

    let source_dir = PathBuf::from(&preparation.plan.backup.path)
        .canonicalize()
        .map_err(|error| format!("无法解析恢复来源目录: {error}"))?;
    let rollback_dir = PathBuf::from(&preparation.rollback_backup.path)
        .canonicalize()
        .map_err(|error| format!("无法解析外部回滚点目录: {error}"))?;
    if paths_equal(&source_dir, &rollback_dir) {
        return Err("恢复来源与外部回滚点不能是同一目录".into());
    }
    if path_is_within(&source_dir, &app_data_dir) || path_is_within(&rollback_dir, &app_data_dir) {
        return Err("恢复来源和外部回滚点都必须位于应用数据目录之外".into());
    }
    let receipt_parent = receipt_path
        .parent()
        .ok_or_else(|| "恢复准备凭据缺少父目录".to_string())?;
    let rollback_parent = rollback_dir
        .parent()
        .ok_or_else(|| "外部回滚点缺少父目录".to_string())?;
    if !paths_equal(receipt_parent, rollback_parent) {
        return Err("恢复准备凭据必须与外部回滚点位于同一父目录".into());
    }

    let source_backup = inspect_user_data_backup(&source_dir, current_app_version)?;
    let rollback_backup = inspect_user_data_backup(&rollback_dir, current_app_version)?;
    ensure_backup_inspection_matches("恢复来源", &preparation.plan.backup, &source_backup)?;
    ensure_backup_inspection_matches("外部回滚点", &preparation.rollback_backup, &rollback_backup)?;

    let source_manifest_sha256 = sha256_file(&source_dir.join("manifest.json"))?;
    let rollback_manifest_sha256 = sha256_file(&rollback_dir.join("manifest.json"))?;
    if source_manifest_sha256 != preparation.source_manifest_sha256 {
        return Err("恢复来源 manifest 摘要与准备凭据不一致".into());
    }
    if rollback_manifest_sha256 != preparation.rollback_manifest_sha256 {
        return Err("外部回滚点 manifest 摘要与准备凭据不一致".into());
    }

    let required_staging_bytes = source_backup.total_bytes;
    let safety_margin_bytes = (required_staging_bytes / 10).max(MIN_RESTORE_SAFETY_MARGIN_BYTES);
    let required_total_bytes = required_staging_bytes
        .checked_add(safety_margin_bytes)
        .ok_or_else(|| "恢复 staging 空间需求溢出".to_string())?;
    let mut blocked_reasons = Vec::new();
    if source_backup.newer_than_current_app {
        blocked_reasons.push(format!(
            "恢复来源来自较新的应用版本 v{}",
            source_backup.source_app_version
        ));
    }
    if target_available_bytes < required_total_bytes {
        blocked_reasons.push(format!(
            "应用数据所在磁盘可用空间不足：需要至少 {required_total_bytes} 字节，当前 {target_available_bytes} 字节"
        ));
    }
    let preflight_passed = blocked_reasons.is_empty();

    Ok(UserDataRestorePreflight {
        schema_version: 1,
        checked_at: now_ms(),
        receipt_path: display_path(&receipt_path),
        source_backup,
        rollback_backup,
        source_manifest_sha256,
        rollback_manifest_sha256,
        target_available_bytes,
        required_staging_bytes,
        safety_margin_bytes,
        required_total_bytes,
        preflight_passed,
        requires_fresh_rollback_at_execution: true,
        restore_authorized: false,
        restore_executed: false,
        blocked_reasons,
        warnings: vec![
            "预检通过不代表用户已授权恢复；执行前仍需明确二次确认".into(),
            "准备后当前数据仍可能变化；真正执行前必须刷新外部回滚点".into(),
        ],
    })
}

fn validate_restore_preparation_shape(
    preparation: &UserDataRestorePreparation,
) -> Result<(), String> {
    if preparation.schema_version != 1 {
        return Err(format!(
            "不支持的恢复准备凭据 schemaVersion: {}",
            preparation.schema_version
        ));
    }
    if !preparation.requires_restart
        || preparation.restore_executed
        || !preparation.plan.requires_restart
        || !preparation.plan.requires_pre_restore_backup
        || !preparation.plan.version_compatible
        || !preparation.plan.blocked_reasons.is_empty()
    {
        return Err("恢复准备凭据状态不允许进入预检".into());
    }
    if !is_sha256_hex(&preparation.source_manifest_sha256)
        || !is_sha256_hex(&preparation.rollback_manifest_sha256)
    {
        return Err("恢复准备凭据中的 manifest 摘要格式无效".into());
    }
    Ok(())
}

fn ensure_backup_inspection_matches(
    label: &str,
    recorded: &UserDataBackupInspection,
    inspected: &UserDataBackupInspection,
) -> Result<(), String> {
    let recorded_path = PathBuf::from(&recorded.path)
        .canonicalize()
        .map_err(|error| format!("无法解析凭据记录的{label}路径: {error}"))?;
    let inspected_path = PathBuf::from(&inspected.path)
        .canonicalize()
        .map_err(|error| format!("无法解析复核后的{label}路径: {error}"))?;
    let immutable_fields_match = recorded.schema_version == inspected.schema_version
        && recorded.source_app_version == inspected.source_app_version
        && recorded.created_at == inspected.created_at
        && recorded.file_count == inspected.file_count
        && recorded.total_bytes == inspected.total_bytes
        && recorded.library_book_count == inspected.library_book_count
        && recorded.reading_progress_count == inspected.reading_progress_count
        && recorded.annotation_count == inspected.annotation_count
        && recorded.plugin_count == inspected.plugin_count
        && recorded.epub_file_count == inspected.epub_file_count;
    if !paths_equal(&recorded_path, &inspected_path) || !immutable_fields_match {
        return Err(format!("{label}摘要与恢复准备凭据不一致"));
    }
    Ok(())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn ensure_restore_plan_is_compatible(plan: &UserDataRestorePlan) -> Result<(), String> {
    if plan.version_compatible && plan.blocked_reasons.is_empty() {
        return Ok(());
    }
    let reasons = if plan.blocked_reasons.is_empty() {
        "恢复计划未通过版本兼容检查".to_string()
    } else {
        plan.blocked_reasons.join("；")
    };
    Err(format!("恢复准备已阻断: {reasons}"))
}

fn write_restore_preparation_receipt(
    partial_path: &Path,
    final_path: &Path,
    preparation: &UserDataRestorePreparation,
) -> Result<(), String> {
    let mut json = serde_json::to_vec_pretty(preparation)
        .map_err(|error| format!("序列化恢复准备凭据失败: {error}"))?;
    json.push(b'\n');
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(partial_path)
            .map_err(|error| format!("创建恢复准备临时凭据失败: {error}"))?;
        file.write_all(&json)
            .map_err(|error| format!("写入恢复准备凭据失败: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("同步恢复准备凭据失败: {error}"))?;
        drop(file);
        fs::rename(partial_path, final_path)
            .map_err(|error| format!("完成恢复准备凭据失败: {error}"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(partial_path);
    }
    result
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

    let manifest_paths = manifest
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let plugin_count = plugin_count_from_paths(&manifest_paths);
    let epub_file_count = epub_count_from_paths(&manifest_paths);
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
        plugin_count,
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

fn collect_current_backup_payload_files(app_data_dir: &Path) -> Result<BTreeSet<String>, String> {
    let library_dir = app_data_dir.join("library");
    if !library_dir.is_dir() {
        return Err("当前书库目录不存在，无法规划可靠回滚点".into());
    }
    let mut files = collect_payload_files(app_data_dir, &library_dir)?;
    files.retain(|path| {
        !matches!(
            path.rsplit('/').next(),
            Some("library.sqlite" | "library.sqlite-wal" | "library.sqlite-shm")
        )
    });
    let plugins_dir = app_data_dir.join("plugins");
    if plugins_dir.is_dir() {
        files.extend(collect_payload_files(app_data_dir, &plugins_dir)?);
    }
    Ok(files)
}

fn plugin_count_from_paths(paths: &BTreeSet<String>) -> usize {
    paths
        .iter()
        .filter_map(|path| {
            let parts = path.split('/').collect::<Vec<_>>();
            (parts.len() == 4
                && parts[0] == "plugins"
                && parts[1] == "sources"
                && matches!(parts[3], "install.json" | "manifest.json"))
            .then(|| parts[2].to_string())
        })
        .collect::<BTreeSet<_>>()
        .len()
}

fn epub_count_from_paths(paths: &BTreeSet<String>) -> usize {
    paths
        .iter()
        .filter(|path| path.to_ascii_lowercase().ends_with(".epub"))
        .count()
}

fn sqlite_snapshot_estimate(connection: &Connection) -> Result<u64, String> {
    let page_count: u64 = connection
        .query_row("PRAGMA page_count", [], |row| row.get(0))
        .map_err(|error| format!("读取 SQLite page_count 失败: {error}"))?;
    let page_size: u64 = connection
        .query_row("PRAGMA page_size", [], |row| row.get(0))
        .map_err(|error| format!("读取 SQLite page_size 失败: {error}"))?;
    page_count
        .checked_mul(page_size)
        .ok_or_else(|| "SQLite 快照预计大小溢出".into())
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

fn unique_restore_preparation_paths(
    destination_parent: &Path,
    prepared_at: i64,
) -> (PathBuf, PathBuf) {
    for suffix in 0_u32.. {
        let name = if suffix == 0 {
            format!("{RESTORE_PREPARATION_PREFIX}-{prepared_at}.json")
        } else {
            format!("{RESTORE_PREPARATION_PREFIX}-{prepared_at}-{suffix}.json")
        };
        let final_path = destination_parent.join(&name);
        let partial_path = destination_parent.join(format!(".{name}.partial"));
        if !final_path.exists() && !partial_path.exists() {
            return (partial_path, final_path);
        }
    }
    unreachable!("u32 restore preparation suffix space exhausted")
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
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

    #[test]
    fn plans_full_replacement_and_required_rollback_without_writing() {
        let (source, storage, library) = fixture();
        let destination = TestDir::new("restore-plan");
        let exported =
            export_user_data_backup(&storage, &library, &source.0, &destination.0, "0.7.3")
                .unwrap();
        let backup = PathBuf::from(&exported.path);
        let manifest_before = fs::read(backup.join("manifest.json")).unwrap();

        let plan = plan_user_data_restore(&storage, &library, &source.0, &backup, "0.7.3").unwrap();

        assert_eq!(plan.current_library_book_count, 1);
        assert_eq!(plan.current_reading_progress_count, 1);
        assert_eq!(plan.current_annotation_count, 1);
        assert_eq!(plan.current_plugin_count, 1);
        assert_eq!(plan.current_epub_file_count, 1);
        assert_eq!(plan.replacement_file_count, exported.file_count);
        assert!(plan.rollback_estimated_bytes > 0);
        assert!(plan.requires_restart);
        assert!(plan.requires_pre_restore_backup);
        assert!(plan.version_compatible);
        assert!(plan.blocked_reasons.is_empty());
        assert_eq!(plan.warnings.len(), 2);
        assert_eq!(
            fs::read(backup.join("manifest.json")).unwrap(),
            manifest_before
        );
    }

    #[test]
    fn restore_plan_blocks_newer_backup_and_app_data_source() {
        let (source, storage, library) = fixture();
        let destination = TestDir::new("restore-plan-newer");
        let exported =
            export_user_data_backup(&storage, &library, &source.0, &destination.0, "9.0.0")
                .unwrap();
        let plan = plan_user_data_restore(
            &storage,
            &library,
            &source.0,
            Path::new(&exported.path),
            "0.7.3",
        )
        .unwrap();
        assert!(!plan.version_compatible);
        assert_eq!(plan.blocked_reasons.len(), 1);
        assert!(plan.blocked_reasons[0].contains("较新的应用版本"));

        let error = plan_user_data_restore(
            &storage,
            &library,
            &source.0,
            &source.0.join("library"),
            "0.7.3",
        )
        .unwrap_err();
        assert!(error.contains("不能位于应用数据目录内部"));
    }

    #[test]
    fn prepares_verified_external_rollback_and_receipt_without_restoring() {
        let (source, storage, library) = fixture();
        let source_parent = TestDir::new("prepare-source");
        let exported =
            export_user_data_backup(&storage, &library, &source.0, &source_parent.0, "0.7.3")
                .unwrap();
        let source_backup = PathBuf::from(&exported.path);

        storage
            .execute("INSERT INTO reading_state VALUES (?1)", params!["book-2"])
            .unwrap();
        fs::write(
            source.0.join("library/objects/book-2.epub"),
            b"new-current-data",
        )
        .unwrap();
        let rollback_parent = TestDir::new("prepare-rollback");

        let preparation = prepare_user_data_restore(
            &storage,
            &library,
            &source.0,
            &source_backup,
            &rollback_parent.0,
            "0.7.3",
        )
        .unwrap();

        assert_eq!(preparation.schema_version, 1);
        assert!(preparation.requires_restart);
        assert!(!preparation.restore_executed);
        assert_eq!(preparation.plan.backup.path, exported.path);
        assert_eq!(preparation.plan.backup.reading_progress_count, 1);
        assert_eq!(preparation.rollback_backup.reading_progress_count, 2);
        assert_eq!(preparation.plan.backup.epub_file_count, 1);
        assert_eq!(preparation.rollback_backup.epub_file_count, 2);
        assert_eq!(preparation.source_manifest_sha256.len(), 64);
        assert_eq!(preparation.rollback_manifest_sha256.len(), 64);
        assert!(Path::new(&preparation.rollback_backup.path).is_dir());
        let receipt: UserDataRestorePreparation =
            serde_json::from_slice(&fs::read(&preparation.receipt_path).unwrap()).unwrap();
        assert_eq!(receipt, preparation);

        let preflight = preflight_user_data_restore_with_available_bytes(
            Path::new(&preparation.receipt_path),
            &source.0,
            "0.7.3",
            u64::MAX,
        )
        .unwrap();
        assert!(preflight.preflight_passed);
        assert!(preflight.blocked_reasons.is_empty());
        assert_eq!(preflight.source_backup.path, preparation.plan.backup.path);
        assert_eq!(
            preflight.rollback_backup.path,
            preparation.rollback_backup.path
        );
        assert_eq!(preflight.target_available_bytes, u64::MAX);
        assert_eq!(
            preflight.required_staging_bytes,
            preparation.plan.backup.total_bytes
        );
        assert_eq!(
            preflight.safety_margin_bytes,
            MIN_RESTORE_SAFETY_MARGIN_BYTES
        );
        assert!(preflight.requires_fresh_rollback_at_execution);
        assert!(!preflight.restore_authorized);
        assert!(!preflight.restore_executed);

        let insufficient = preflight_user_data_restore_with_available_bytes(
            Path::new(&preparation.receipt_path),
            &source.0,
            "0.7.3",
            0,
        )
        .unwrap();
        assert!(!insufficient.preflight_passed);
        assert_eq!(insufficient.blocked_reasons.len(), 1);
        assert!(insufficient.blocked_reasons[0].contains("可用空间不足"));
        assert_eq!(table_count(&storage, "reading_state").unwrap(), 2);
        assert!(source.0.join("library/objects/book-2.epub").is_file());
    }

    #[test]
    fn preflight_rejects_tampered_receipt_and_manifest_drift() {
        let (source, storage, library) = fixture();
        let source_parent = TestDir::new("preflight-source");
        let exported =
            export_user_data_backup(&storage, &library, &source.0, &source_parent.0, "0.7.3")
                .unwrap();
        let rollback_parent = TestDir::new("preflight-rollback");
        let preparation = prepare_user_data_restore(
            &storage,
            &library,
            &source.0,
            Path::new(&exported.path),
            &rollback_parent.0,
            "0.7.3",
        )
        .unwrap();
        let receipt_path = PathBuf::from(&preparation.receipt_path);

        let mut tampered = preparation.clone();
        tampered.source_manifest_sha256 = "0".repeat(64);
        fs::write(&receipt_path, serde_json::to_vec_pretty(&tampered).unwrap()).unwrap();
        let error = preflight_user_data_restore_with_available_bytes(
            &receipt_path,
            &source.0,
            "0.7.3",
            u64::MAX,
        )
        .unwrap_err();
        assert!(error.contains("来源 manifest 摘要"));

        fs::write(
            &receipt_path,
            serde_json::to_vec_pretty(&preparation).unwrap(),
        )
        .unwrap();
        fs::write(
            Path::new(&exported.path).join("library/objects/book.epub"),
            b"changed-after-preparation",
        )
        .unwrap();
        let error = preflight_user_data_restore_with_available_bytes(
            &receipt_path,
            &source.0,
            "0.7.3",
            u64::MAX,
        )
        .unwrap_err();
        assert!(error.contains("校验失败"));
    }

    #[test]
    fn preparation_rejects_blocked_source_and_nested_rollback_target() {
        let (source, storage, library) = fixture();
        let source_parent = TestDir::new("prepare-blocked-source");
        let exported =
            export_user_data_backup(&storage, &library, &source.0, &source_parent.0, "9.0.0")
                .unwrap();
        let rollback_parent = TestDir::new("prepare-blocked-rollback");
        let error = prepare_user_data_restore(
            &storage,
            &library,
            &source.0,
            Path::new(&exported.path),
            &rollback_parent.0,
            "0.7.3",
        )
        .unwrap_err();
        assert!(error.contains("恢复准备已阻断"));
        assert_eq!(fs::read_dir(&rollback_parent.0).unwrap().count(), 0);

        let compatible_parent = TestDir::new("prepare-nested-source");
        let compatible =
            export_user_data_backup(&storage, &library, &source.0, &compatible_parent.0, "0.7.3")
                .unwrap();
        let compatible_path = PathBuf::from(&compatible.path);
        let error = prepare_user_data_restore(
            &storage,
            &library,
            &source.0,
            &compatible_path,
            &compatible_path,
            "0.7.3",
        )
        .unwrap_err();
        assert!(error.contains("不能位于恢复来源目录内部"));
    }
}
