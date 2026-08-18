//! 自托管同步服务器 —— 用户在自己的 NAS/VPS 上运行。
//!
//! 启动：`sync-server --db ./sync.db --bind 0.0.0.0:9876`
//!
//! 不做账号系统：用 library_id + pairing_secret 做设备配对，
//! 一个实例可挂多个独立 library（数据严格隔离）。

use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post, put},
    Router,
};
use futures::{SinkExt, StreamExt};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

// ---- 类型 ----

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChangesQuery {
    since: Option<u64>,
    library_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PairRequest {
    library_id: Option<String>, // None = create new library
    device_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PairJoinRequest {
    pairing_code: String,
    device_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PairResponse {
    library_id: String,
    pairing_code: String,
    token: String, // bearer token for this device
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PushRequest {
    library_id: String,
    changes: Vec<ChangeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChangeEntry {
    entity_type: String,
    entity_id: String,
    op: String, // "upsert" | "delete"
    payload: String,
    device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotResponse {
    editions: Vec<String>, // JSON rows
    assets: Vec<String>,
    annotations: Vec<String>,
    reading_states: Vec<String>,
    blob_hashes: Vec<String>, // SHA-256 of EPUB files on server
}

// ---- App State ----

struct AppState {
    db: Mutex<Connection>,
    /// library_id → broadcast sender for WS push
    ws_tx: Mutex<HashMap<String, broadcast::Sender<String>>>,
}

impl AppState {
    fn new(db_path: &str) -> Self {
        let conn = Connection::open(db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS libraries (
                library_id TEXT PRIMARY KEY,
                pairing_secret TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS devices (
                token TEXT PRIMARY KEY,
                library_id TEXT NOT NULL,
                device_name TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sync_data (
                cursor INTEGER PRIMARY KEY AUTOINCREMENT,
                library_id TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                op TEXT NOT NULL,
                payload TEXT NOT NULL,
                device_id TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS blobs (
                hash TEXT PRIMARY KEY,
                library_id TEXT NOT NULL,
                data BLOB NOT NULL,
                size INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_sync_library_cursor ON sync_data(library_id, cursor);",
        )
        .unwrap();

        Self {
            db: Mutex::new(conn),
            ws_tx: Mutex::new(HashMap::new()),
        }
    }

    fn get_or_create_ws_tx(&self, library_id: &str) -> broadcast::Sender<String> {
        let mut map = self.ws_tx.lock().unwrap();
        map.entry(library_id.to_string())
            .or_insert_with(|| broadcast::channel(256).0)
            .clone()
    }
}

// ---- Handlers ----

type SharedState = Arc<AppState>;

#[tokio::main]
async fn main() {
    let db_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "sync.db".to_string());
    let bind = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "0.0.0.0:9876".to_string());

    let state = Arc::new(AppState::new(&db_path));

    let app = Router::new()
        .route("/pair", post(handle_pair))
        .route("/pair/join", post(handle_pair_join))
        .route("/sync/changes", get(handle_changes))
        .route("/sync/snapshot", get(handle_snapshot))
        .route("/sync/push", post(handle_push))
        .route("/sync/blobs/{hash}", put(handle_put_blob))
        .route("/sync/blobs/{hash}", get(handle_get_blob))
        .route("/sync/ws", get(handle_ws))
        .layer(CorsLayer::permissive())
        .with_state(state);

    println!("Sync server listening on {}", bind);
    let listener = tokio::net::TcpListener::bind(&bind).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// -- Pairing --

async fn handle_pair(
    State(state): State<SharedState>,
    Json(req): Json<PairRequest>,
) -> impl IntoResponse {
    let library_id = req
        .library_id
        .unwrap_or_else(|| reading_core::sync::generate_library_id());
    let secret = reading_core::sync::generate_pairing_secret();
    let pairing_code = &secret[..6]; // 6-digit code
    let token = uuid::Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let db = state.db.lock().unwrap();
    db.execute(
        "INSERT OR IGNORE INTO libraries (library_id, pairing_secret, created_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![library_id, secret, now],
    )
    .unwrap();
    db.execute(
        "UPDATE libraries SET pairing_secret = ?1 WHERE library_id = ?2",
        rusqlite::params![secret, library_id],
    )
    .unwrap();
    db.execute(
        "INSERT INTO devices (token, library_id, device_name, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![token, library_id, req.device_name, now],
    )
    .unwrap();

    Json(PairResponse {
        library_id: library_id.clone(),
        pairing_code: pairing_code.to_string(),
        token,
    })
}

async fn handle_pair_join(
    State(state): State<SharedState>,
    Json(req): Json<PairJoinRequest>,
) -> Result<Json<PairResponse>, StatusCode> {
    let db = state.db.lock().unwrap();
    let secret: String = db
        .query_row(
            "SELECT pairing_secret FROM libraries WHERE pairing_secret LIKE ?1 || '%'",
            rusqlite::params![req.pairing_code],
            |r| r.get(0),
        )
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let library_id: String = db
        .query_row(
            "SELECT library_id FROM libraries WHERE pairing_secret = ?1",
            rusqlite::params![secret],
            |r| r.get(0),
        )
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let token = uuid::Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    db.execute(
        "INSERT INTO devices (token, library_id, device_name, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![token, library_id, req.device_name, now],
    )
    .unwrap();

    Ok(Json(PairResponse {
        library_id,
        pairing_code: req.pairing_code,
        token,
    }))
}

fn verify_token(db: &Connection, token: &str, library_id: &str) -> bool {
    db.query_row(
        "SELECT 1 FROM devices WHERE token = ?1 AND library_id = ?2",
        rusqlite::params![token, library_id],
        |_| Ok(()),
    )
    .is_ok()
}

fn extract_auth(headers: &axum::http::HeaderMap) -> Option<(String, String)> {
    let auth = headers.get("authorization")?.to_str().ok()?;
    let token = auth.strip_prefix("Bearer ")?;
    // library_id comes from query param, not header
    Some((token.to_string(), String::new()))
}

// -- Sync endpoints --

async fn handle_changes(
    State(state): State<SharedState>,
    Query(q): Query<ChangesQuery>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    let (token, _) = extract_auth(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let library_id = q.library_id.as_deref().ok_or(StatusCode::BAD_REQUEST)?;
    let db = state.db.lock().unwrap();

    if !verify_token(&db, &token, library_id) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let since = q.since.unwrap_or(0);
    let mut stmt = db
        .prepare(
            "SELECT cursor, entity_type, entity_id, op, payload, device_id, created_at
             FROM sync_data WHERE library_id = ?1 AND cursor > ?2 ORDER BY cursor LIMIT 500",
        )
        .unwrap();

    let rows: Vec<serde_json::Value> = stmt
        .query_map(rusqlite::params![library_id, since], |row| {
            Ok(serde_json::json!({
                "cursor": row.get::<_, i64>(0)?,
                "entity_type": row.get::<_, String>(1)?,
                "entity_id": row.get::<_, String>(2)?,
                "op": row.get::<_, String>(3)?,
                "payload": row.get::<_, String>(4)?,
                "device_id": row.get::<_, String>(5)?,
                "created_at": row.get::<_, i64>(6)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(rows))
}

async fn handle_snapshot(
    State(state): State<SharedState>,
    Query(q): Query<ChangesQuery>,
    headers: axum::http::HeaderMap,
) -> Result<Json<SnapshotResponse>, StatusCode> {
    let (token, _) = extract_auth(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let library_id = q.library_id.as_deref().ok_or(StatusCode::BAD_REQUEST)?;
    let db = state.db.lock().unwrap();

    if !verify_token(&db, &token, library_id) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Aggregate current state: group by entity_type, take latest upsert for each entity_id
    // (This is a simplified MVP — production should use a proper materialized view)
    let editions: Vec<String> = snapshot_entities(&db, library_id, "edition");
    let annotations: Vec<String> = snapshot_entities(&db, library_id, "annotation");
    let reading_states: Vec<String> = snapshot_entities(&db, library_id, "reading_state");
    let assets: Vec<String> = snapshot_entities(&db, library_id, "asset");

    let blob_hashes: Vec<String> = db
        .prepare("SELECT hash FROM blobs WHERE library_id = ?1")
        .unwrap()
        .query_map(rusqlite::params![library_id], |r| r.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(SnapshotResponse {
        editions,
        assets,
        annotations,
        reading_states,
        blob_hashes,
    }))
}

fn snapshot_entities(db: &Connection, library_id: &str, entity_type: &str) -> Vec<String> {
    let mut stmt = db
        .prepare(
            "SELECT payload FROM sync_data
             WHERE library_id = ?1 AND entity_type = ?2 AND op = 'upsert'
             AND cursor IN (
                 SELECT MAX(cursor) FROM sync_data
                 WHERE library_id = ?1 AND entity_type = ?2 AND op = 'upsert'
                 GROUP BY entity_id
             )",
        )
        .unwrap();
    stmt.query_map(rusqlite::params![library_id, entity_type], |r| r.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
}

async fn handle_push(
    State(state): State<SharedState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<PushRequest>,
) -> Result<StatusCode, StatusCode> {
    let (token, _) = extract_auth(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let db = state.db.lock().unwrap();

    if !verify_token(&db, &token, &req.library_id) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let mut stmt = db
        .prepare(
            "INSERT INTO sync_data (library_id, entity_type, entity_id, op, payload, device_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .unwrap();

    for change in &req.changes {
        stmt.execute(rusqlite::params![
            req.library_id,
            change.entity_type,
            change.entity_id,
            change.op,
            change.payload,
            change.device_id,
            now,
        ])
        .unwrap();
    }

    // Notify WebSocket subscribers
    if let Ok(tx) = state.ws_tx.lock() {
        if let Some(tx) = tx.get(&req.library_id) {
            let _ = tx.send(serde_json::to_string(&req.changes).unwrap_or_default());
        }
    }

    Ok(StatusCode::OK)
}

async fn handle_put_blob(
    State(state): State<SharedState>,
    Path(hash): Path<String>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<StatusCode, StatusCode> {
    let (token, _) = extract_auth(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    // library_id from query or header — simplified: use token to find library
    let db = state.db.lock().unwrap();
    let library_id: String = db
        .query_row(
            "SELECT library_id FROM devices WHERE token = ?1",
            rusqlite::params![token],
            |r| r.get(0),
        )
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    db.execute(
        "INSERT OR REPLACE INTO blobs (hash, library_id, data, size) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![hash, library_id, body.as_ref(), body.len()],
    )
    .unwrap();

    Ok(StatusCode::OK)
}

async fn handle_get_blob(
    State(state): State<SharedState>,
    Path(hash): Path<String>,
) -> Result<Vec<u8>, StatusCode> {
    let db = state.db.lock().unwrap();
    let data: Vec<u8> = db
        .query_row(
            "SELECT data FROM blobs WHERE hash = ?1",
            rusqlite::params![hash],
            |r| r.get(0),
        )
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(data)
}

// -- WebSocket --

async fn handle_ws(
    State(state): State<SharedState>,
    Query(q): Query<ChangesQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let library_id = q.library_id.unwrap_or_default();
    let tx = state.get_or_create_ws_tx(&library_id);
    let mut rx = tx.subscribe();

    ws.on_upgrade(move |socket| async move {
        let (mut sender, mut _receiver) = socket.split();
        while let Ok(msg) = rx.recv().await {
            if sender
                .send(axum::extract::ws::Message::Text(msg.into()))
                .await
                .is_err()
            {
                break;
            }
        }
    })
}
