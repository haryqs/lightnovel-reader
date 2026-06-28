//! Tauri sync 命令 —— 桌面端对接自托管同步服务器。
//!
//! 凭据保存在 app data 目录的 sync.json 中。

use std::path::PathBuf;

use crate::BridgeError;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
struct SyncCredential {
    library_id: String,
    token: String,
    server_url: String,
    device_name: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatusOut {
    paired: bool,
    last_sync_at: Option<i64>,
    pending_changes: u64,
    library_id: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPairResult {
    library_id: String,
    pairing_code: String,
    token: String,
}

fn sync_cred_path(app_data: &PathBuf) -> PathBuf {
    app_data.join("sync.json")
}

fn load_cred(app_data: &PathBuf) -> Option<SyncCredential> {
    let path = sync_cred_path(app_data);
    if !path.exists() { return None; }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn save_cred(app_data: &PathBuf, cred: &SyncCredential) -> Result<(), String> {
    let json = serde_json::to_string_pretty(cred).map_err(|e| e.to_string())?;
    std::fs::write(sync_cred_path(app_data), &json).map_err(|e| e.to_string())
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("reqwest client")
}

// ---- Commands ----

#[tauri::command]
pub fn sync_status(app_data: tauri::State<'_, PathBuf>) -> SyncStatusOut {
    let cred = load_cred(&app_data);
    SyncStatusOut {
        paired: cred.is_some(),
        last_sync_at: None,
        pending_changes: 0,
        library_id: cred.as_ref().map(|c| c.library_id.clone()),
    }
}

/// 首次配对：创建新 library 并返回配对码。
#[tauri::command]
pub fn sync_pair(
    app_data: tauri::State<'_, PathBuf>,
    server_url: String,
    device_name: Option<String>,
) -> Result<SyncPairResult, BridgeError> {
    let url = format!("{}/pair", server_url.trim_end_matches('/'));
    let resp = client()
        .post(&url)
        .json(&serde_json::json!({
            "device_name": device_name.unwrap_or_else(|| "desktop".into())
        }))
        .send()
        .map_err(|e| BridgeError::network(format!("连接同步服务器失败: {e}")))?;

    if !resp.status().is_success() {
        return Err(BridgeError::network("同步服务器返回错误"));
    }

    let body: serde_json::Value = resp.json().map_err(|e| BridgeError::parse(e.to_string()))?;
    let cred = SyncCredential {
        library_id: body["library_id"].as_str().unwrap_or("").into(),
        token: body["token"].as_str().unwrap_or("").into(),
        server_url,
        device_name: "desktop".into(),
    };
    save_cred(&app_data, &cred).map_err(|e| BridgeError::storage(e))?;

    Ok(SyncPairResult {
        library_id: cred.library_id.clone(),
        pairing_code: body["pairing_code"].as_str().unwrap_or("").into(),
        token: cred.token.clone(),
    })
}

/// 用配对码加入已有 library。
#[tauri::command]
pub fn sync_pair_join(
    app_data: tauri::State<'_, PathBuf>,
    server_url: String,
    pairing_code: String,
    device_name: Option<String>,
) -> Result<SyncPairResult, BridgeError> {
    let url = format!("{}/pair/join", server_url.trim_end_matches('/'));
    let resp = client()
        .post(&url)
        .json(&serde_json::json!({
            "pairing_code": pairing_code,
            "device_name": device_name.unwrap_or_else(|| "desktop".into())
        }))
        .send()
        .map_err(|e| BridgeError::network(format!("连接同步服务器失败: {e}")))?;

    if !resp.status().is_success() {
        return Err(BridgeError::forbidden("配对码无效或已过期"));
    }

    let body: serde_json::Value = resp.json().map_err(|e| BridgeError::parse(e.to_string()))?;
    let cred = SyncCredential {
        library_id: body["library_id"].as_str().unwrap_or("").into(),
        token: body["token"].as_str().unwrap_or("").into(),
        server_url,
        device_name: "desktop".into(),
    };
    save_cred(&app_data, &cred).map_err(|e| BridgeError::storage(e))?;

    Ok(SyncPairResult {
        library_id: cred.library_id.clone(),
        pairing_code: body["pairing_code"].as_str().unwrap_or("").into(),
        token: cred.token.clone(),
    })
}

/// 取消配对。
#[tauri::command]
pub fn sync_unpair(app_data: tauri::State<'_, PathBuf>) -> Result<(), BridgeError> {
    let path = sync_cred_path(&app_data);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| BridgeError::storage(e.to_string()))?;
    }
    Ok(())
}

/// 推送本地变更到服务器。
#[tauri::command]
pub fn sync_push(
    app_data: tauri::State<'_, PathBuf>,
    changes: Vec<serde_json::Value>,
) -> Result<(), BridgeError> {
    let cred = load_cred(&app_data).ok_or_else(|| BridgeError::forbidden("未配对"))?;
    let url = format!("{}/sync/push", cred.server_url.trim_end_matches('/'));

    let resp = client()
        .post(&url)
        .header("Authorization", format!("Bearer {}", cred.token))
        .json(&serde_json::json!({
            "library_id": cred.library_id,
            "changes": changes,
        }))
        .send()
        .map_err(|e| BridgeError::network(format!("推送失败: {e}")))?;

    if !resp.status().is_success() {
        return Err(BridgeError::network("同步服务器拒绝推送"));
    }
    Ok(())
}

/// 拉取远程变更。
#[tauri::command]
pub fn sync_pull(
    app_data: tauri::State<'_, PathBuf>,
    since: Option<u64>,
) -> Result<Vec<serde_json::Value>, BridgeError> {
    let cred = load_cred(&app_data).ok_or_else(|| BridgeError::forbidden("未配对"))?;
    let mut url = format!("{}/sync/changes?library_id={}", 
        cred.server_url.trim_end_matches('/'), cred.library_id);
    if let Some(c) = since { url.push_str(&format!("&since={}", c)); }

    let resp = client()
        .get(&url)
        .header("Authorization", format!("Bearer {}", cred.token))
        .send()
        .map_err(|e| BridgeError::network(format!("拉取失败: {e}")))?;

    let body: Vec<serde_json::Value> = resp.json().map_err(|e| BridgeError::parse(e.to_string()))?;
    Ok(body)
}
