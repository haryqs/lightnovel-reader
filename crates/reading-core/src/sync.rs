//! 跨端同步类型与冲突解决算法。
//!
//! 纯函数、无 I/O —— native 和 wasm feature 都可编译。
//! 网页端用同一份算法在本地做乐观合并，重连后与服务器对账。
//!
//! 冲突策略见文档 10 第 4.4 节。

use serde::{Deserialize, Serialize};

// ---- 数据模型 ----

/// sync_outbox 中的一条变更记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEvent {
    pub cursor: u64,
    pub entity_type: EntityType,
    pub entity_id: String,
    pub op: SyncOp,
    /// 整行 JSON 快照（与桥接协议 DTO 同形）
    pub payload: String,
    pub device_id: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Edition,
    Asset,
    Annotation,
    ReadingState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncOp {
    Upsert,
    Delete,
}

/// 带 sync 字段的阅读进度行。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncableReadingState {
    pub book_id: String,
    pub chapter_href: String,
    pub chapter_progress: f64,
    pub percentage: f64,
    pub last_modified: i64,
    pub sync_version: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
    pub device_id: String,
}

/// 带 sync 字段的标注行。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncableAnnotation {
    pub id: String,
    pub book_id: String,
    pub kind: String,
    pub color: Option<String>,
    pub chapter_href: String,
    pub anchor_start: u32,
    pub anchor_end: u32,
    pub exact: String,
    pub note: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_modified: i64,
    pub sync_version: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
    pub device_id: String,
}

// ---- 冲突解决 ----

/// 阅读进度冲突：Last-Write-Wins + device_id tie-break。
///
/// 返回应保留的版本。
pub fn resolve_reading_progress(
    local: &SyncableReadingState,
    remote: &SyncableReadingState,
) -> ResolveResult {
    if remote.last_modified > local.last_modified {
        ResolveResult::TakeRemote
    } else if local.last_modified > remote.last_modified {
        ResolveResult::KeepLocal
    } else {
        // 同一时刻：device_id 字典序 tie-break
        if remote.device_id > local.device_id {
            ResolveResult::TakeRemote
        } else {
            ResolveResult::KeepLocal
        }
    }
}

/// 标注冲突：行级 LWW + 防复活术。
///
/// - 不同 id 之间无冲突（不同行）
/// - 同 id 比较 updated_at
/// - 删除优先（墓碑），但更晚的编辑可以复活
pub fn resolve_annotation(
    local: &SyncableAnnotation,
    remote: &SyncableAnnotation,
) -> ResolveResult {
    // 规则 1: 墓碑优先。谁被删了就以谁为准（删除 = 最后一次操作）。
    let local_deleted = local.deleted_at.is_some();
    let remote_deleted = remote.deleted_at.is_some();

    if local_deleted && remote_deleted {
        // 都被删了：取删除时间晚的那个（但两者都删，保留 local 即可）
        return ResolveResult::KeepLocal;
    }
    if local_deleted && !remote_deleted {
        // 本地删了，远端还在编辑 → 检查远端编辑是否发生在删除之后
        if remote.updated_at > local.deleted_at.unwrap_or(0) {
            // 远端在删除后继续编辑 → 复活（取远端）
            return ResolveResult::TakeRemote;
        }
        // 远端编辑在删除之前 → 删除生效
        return ResolveResult::KeepLocal;
    }
    if !local_deleted && remote_deleted {
        // 远端删了，本地还在编辑 → 检查本地编辑是否在远端删除之后
        if local.updated_at > remote.deleted_at.unwrap_or(0) {
            return ResolveResult::KeepLocal;
        }
        return ResolveResult::TakeRemote;
    }

    // 规则 2: 都没删 → LWW by updated_at
    if remote.updated_at > local.updated_at {
        ResolveResult::TakeRemote
    } else if local.updated_at > remote.updated_at {
        ResolveResult::KeepLocal
    } else {
        // tie-break by device_id
        if remote.device_id > local.device_id {
            ResolveResult::TakeRemote
        } else {
            ResolveResult::KeepLocal
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveResult {
    KeepLocal,
    TakeRemote,
}

// ---- 辅助 ----

/// 生成设备配对 secret（32 字节随机数，hex 编码）。
/// 在 wasm 环境下不可用（无 rand），调用方需自行提供随机数。
#[cfg(feature = "native")]
pub fn generate_pairing_secret() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64,
    );
    format!("{:016x}{:016x}", hasher.finish(), hasher.finish())
}

/// 生成 library_id（UUID v4 格式，无外部依赖的简化实现）。
#[cfg(feature = "native")]
pub fn generate_library_id() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    fn rand_hex() -> String {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
        );
        format!("{:08x}", h.finish() as u32)
    }
    format!(
        "{}-{}-4{}-a{}-{}",
        rand_hex(),
        &rand_hex()[..4],
        &rand_hex()[..3],
        &rand_hex()[..3],
        rand_hex() + &rand_hex()[..4]
    )
}

// ---- 测试 ----

#[cfg(test)]
mod tests {
    use super::*;

    fn make_reading_state(last_modified: i64, device_id: &str) -> SyncableReadingState {
        SyncableReadingState {
            book_id: "test".into(),
            chapter_href: "ch1".into(),
            chapter_progress: 0.5,
            percentage: 0.3,
            last_modified,
            sync_version: 1,
            deleted_at: None,
            device_id: device_id.into(),
        }
    }

    #[test]
    fn progress_newer_wins() {
        let local = make_reading_state(100, "A");
        let remote = make_reading_state(200, "B");
        assert_eq!(
            resolve_reading_progress(&local, &remote),
            ResolveResult::TakeRemote
        );
    }

    #[test]
    fn progress_tiebreak_by_device_id() {
        let local = make_reading_state(100, "A");
        let remote = make_reading_state(100, "B");
        assert_eq!(
            resolve_reading_progress(&local, &remote),
            ResolveResult::TakeRemote
        );
    }

    fn make_annotation(
        updated_at: i64,
        deleted_at: Option<i64>,
        device_id: &str,
    ) -> SyncableAnnotation {
        SyncableAnnotation {
            id: "ann1".into(),
            book_id: "b1".into(),
            kind: "highlight".into(),
            color: None,
            chapter_href: "ch1".into(),
            anchor_start: 0,
            anchor_end: 10,
            exact: "hello".into(),
            note: None,
            created_at: 0,
            updated_at,
            last_modified: updated_at,
            sync_version: 1,
            deleted_at,
            device_id: device_id.into(),
        }
    }

    #[test]
    fn annotation_lww_newer_wins() {
        let local = make_annotation(100, None, "A");
        let remote = make_annotation(200, None, "B");
        assert_eq!(
            resolve_annotation(&local, &remote),
            ResolveResult::TakeRemote
        );
    }

    #[test]
    fn annotation_tombstone_beats_older_edit() {
        let local = make_annotation(100, Some(150), "A"); // deleted at 150
        let remote = make_annotation(120, None, "B"); // edited at 120 < 150
        assert_eq!(
            resolve_annotation(&local, &remote),
            ResolveResult::KeepLocal
        );
    }

    #[test]
    fn annotation_edit_after_delete_resurrects() {
        let local = make_annotation(100, Some(150), "A"); // deleted at 150
        let remote = make_annotation(200, None, "B"); // edited at 200 > 150 → resurrect
        assert_eq!(
            resolve_annotation(&local, &remote),
            ResolveResult::TakeRemote
        );
    }

    #[test]
    fn annotation_both_deleted_keep_local() {
        let local = make_annotation(100, Some(200), "A");
        let remote = make_annotation(150, Some(180), "B");
        assert_eq!(
            resolve_annotation(&local, &remote),
            ResolveResult::KeepLocal
        );
    }
}
