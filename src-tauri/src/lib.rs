use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reading_core::connectors;
use reading_core::epub_parser::{self, BookInfo};
use reading_core::{compute_book_id, library, parse_cache, rusqlite, storage};
use tauri::Manager;

struct LoadedBook {
    book_id: String,     // 内容哈希；持久化解析缓存的 key
    bytes: Arc<Vec<u8>>, // 解码后的 EPUB 原始字节，供按需解析与图片协议复用
    book_info: BookInfo,
    chapters: Mutex<HashMap<String, String>>,
}

/// 极简 percent-decode（图片路径可能含 %20 等），避免引入额外依赖。
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

struct AppState {
    book: Mutex<Option<LoadedBook>>,
    db: Mutex<rusqlite::Connection>,
    library_db: Mutex<rusqlite::Connection>,
    library_dir: std::path::PathBuf,
    cache_dir: std::path::PathBuf, // 持久化解析缓存根目录
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BridgeError {
    code: &'static str,
    message: String,
    details: Option<String>,
}

impl BridgeError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    fn with_details(
        code: &'static str,
        message: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            details: Some(details.into()),
        }
    }

    fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new("invalidArgument", message)
    }

    fn storage(message: impl Into<String>) -> Self {
        Self::new("storageError", message)
    }

    fn parse(message: impl Into<String>) -> Self {
        Self::new("parseError", message)
    }

    fn network(message: impl Into<String>) -> Self {
        Self::new("networkError", message)
    }

    fn http_status(status: reqwest::StatusCode) -> Self {
        Self::with_details("httpStatus", "OPDS 服务器返回错误状态", status.to_string())
    }

    fn http_status_for(label: impl Into<String>, status: reqwest::StatusCode) -> Self {
        Self::with_details("httpStatus", label.into(), status.to_string())
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self::new("notFound", message)
    }

    fn forbidden(message: impl Into<String>) -> Self {
        Self::new("forbidden", message)
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

const AOZORA_CATALOG_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const AOZORA_SEARCH_LIMIT: usize = 40;
const NAROU_SEARCH_LIMIT: usize = 40;
const BANGUMI_SEARCH_LIMIT: usize = 20;

fn resolve_app_data_dir(app: &tauri::App) -> PathBuf {
    const OVERRIDE_ENV: &str = "LIGHTNOVEL_READER_APP_DATA_DIR";

    if let Ok(path) = std::env::var(OVERRIDE_ENV) {
        let path = path.trim();
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }

    app.path().app_data_dir().expect("无法解析 app data 目录")
}

fn load_book_from_data(state: &AppState, data: Vec<u8>) -> Result<OpenedBook, String> {
    let book_id = compute_book_id(&data);
    let cache_root = &state.cache_dir;

    // 解析缓存命中则跳过 OPF/NCX 解析；未命中解析一次并落盘。
    let book_info = match parse_cache::load_book_info(cache_root, &book_id) {
        Some(info) => info,
        None => {
            let info = epub_parser::parse_book_info(&data)?;
            parse_cache::store_book_info(cache_root, &book_id, &info);
            info
        }
    };

    // 预热首章：优先读盘缓存，未命中清洗一次并落盘。
    let mut chapters = HashMap::new();
    if let Some(first) = book_info.spine.first() {
        let href = first.href.clone();
        let html = match parse_cache::load_chapter(cache_root, &book_id, &href) {
            Some(html) => Some(html),
            None => match epub_parser::parse_single_chapter(&data, &href, &book_info) {
                Ok(html) => {
                    parse_cache::store_chapter(cache_root, &book_id, &href, &html);
                    Some(html)
                }
                Err(_) => None,
            },
        };
        if let Some(html) = html {
            chapters.insert(href, html);
        }
    }

    let loaded = LoadedBook {
        book_id: book_id.clone(),
        bytes: Arc::new(data),
        book_info: book_info.clone(),
        chapters: Mutex::new(chapters),
    };
    *state.book.lock().map_err(|e| e.to_string())? = Some(loaded);
    Ok(OpenedBook {
        info: book_info,
        book_id,
    })
}

// async：解析在异步运行时线程上跑，不阻塞 webview 主线程（否则大书开书时 UI"未响应"）。
// 二进制直传，省掉 base64 编解码开销。
#[tauri::command]
async fn open_book_bytes(
    state: tauri::State<'_, AppState>,
    data: Vec<u8>,
) -> Result<BookInfo, String> {
    Ok(load_book_from_data(&state, data)?.info)
}

#[tauri::command]
fn get_chapter(state: tauri::State<AppState>, href: String) -> Result<String, String> {
    if href.trim().is_empty() {
        return Err("章节 href 为空".to_string());
    }
    let book_guard = state.book.lock().map_err(|e| e.to_string())?;
    let book = book_guard.as_ref().ok_or("尚未打开任何书籍")?;

    // 检查缓存
    {
        let chapters = book.chapters.lock().map_err(|e| e.to_string())?;
        if let Some(html) = chapters.get(&href) {
            eprintln!("  缓存命中 (精确)");
            return Ok(html.clone());
        }
        let basename = href.rsplit('/').next().unwrap_or(&href);
        for (key, html) in chapters.iter() {
            let key_basename = key.rsplit('/').next().unwrap_or(key);
            if key_basename == basename {
                eprintln!("  缓存命中 (文件名: {} -> {})", key_basename, key);
                return Ok(html.clone());
            }
        }
    }

    // 内存未命中——先查磁盘解析缓存（已读过的章/二次开书在此命中，跳过清洗）
    if let Some(html) = parse_cache::load_chapter(&state.cache_dir, &book.book_id, &href) {
        book.chapters
            .lock()
            .map_err(|e| e.to_string())?
            .insert(href.clone(), html.clone());
        return Ok(html);
    }

    // 全部未命中——按需解析 + 清洗，写回内存与磁盘缓存
    eprintln!("  缓存未命中，按需解析");
    let html = epub_parser::parse_single_chapter(&book.bytes[..], &href, &book.book_info)?;
    parse_cache::store_chapter(&state.cache_dir, &book.book_id, &href, &html);

    let mut chapters = book.chapters.lock().map_err(|e| e.to_string())?;
    chapters.insert(href.clone(), html.clone());

    Ok(html)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenedBook {
    info: BookInfo,
    book_id: String,
}

// 按路径开书（Calibre 集成）。返回 bookId，使按路径开书与文件选择器开书的标注一致。
#[tauri::command]
async fn open_book_path(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<OpenedBook, String> {
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    load_book_from_data(&state, data)
}

#[derive(serde::Serialize)]
struct CalibreBook {
    title: String,
    author: String,
    path: String, // epub 完整路径
    #[serde(rename = "coverPath")]
    cover_path: String, // Calibre 目录中的 cover.jpg，可能为空
}

// 读取 Calibre 库的 metadata.db，列出全部 EPUB 书（标题/作者/文件路径）。
#[tauri::command]
async fn list_calibre_books(library: String) -> Result<Vec<CalibreBook>, BridgeError> {
    if library.trim().is_empty() {
        return Err(BridgeError::invalid_argument(
            "Calibre library path is required",
        ));
    }
    let db_path = std::path::Path::new(&library).join("metadata.db");
    let conn =
        rusqlite::Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| BridgeError::storage(format!("打开 Calibre 库失败: {}", e)))?;

    let mut stmt = conn
        .prepare(
            "SELECT b.title, b.path, d.name,
                (SELECT a.name FROM authors a
                 JOIN books_authors_link l ON l.author = a.id
                 WHERE l.book = b.id LIMIT 1)
             FROM books b
             JOIN data d ON d.book = b.id AND d.format = 'EPUB'
             ORDER BY b.author_sort, b.sort",
        )
        .map_err(|e| BridgeError::storage(e.to_string()))?;

    let lib = library.clone();
    let rows = stmt
        .query_map([], |r| {
            let title: String = r.get(0)?;
            let path: String = r.get(1)?;
            let name: String = r.get(2)?;
            let author: Option<String> = r.get(3)?;
            let book_dir = std::path::Path::new(&lib).join(&path);
            let full = book_dir
                .join(format!("{}.epub", name))
                .to_string_lossy()
                .to_string();
            let cover = book_dir.join("cover.jpg");
            Ok(CalibreBook {
                title,
                author: author.unwrap_or_default(),
                path: full,
                cover_path: if cover.exists() {
                    cover.to_string_lossy().to_string()
                } else {
                    String::new()
                },
            })
        })
        .map_err(|e| BridgeError::storage(e.to_string()))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| BridgeError::storage(e.to_string()))
}

// —— 自有书库命令 ——

#[tauri::command]
fn library_import(
    state: tauri::State<AppState>,
    path: String,
) -> Result<library::ImportOutcome, BridgeError> {
    if path.trim().is_empty() {
        return Err(BridgeError::invalid_argument("EPUB path is required"));
    }
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    library::import_epub(&db, &state.library_dir, Path::new(&path), now_ms())
        .map_err(BridgeError::storage)
}

#[tauri::command]
fn library_import_bytes(
    state: tauri::State<AppState>,
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<library::ImportOutcome, BridgeError> {
    if data.is_empty() {
        return Err(BridgeError::invalid_argument("EPUB data is empty"));
    }
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    library::import_epub_bytes(
        &db,
        &state.library_dir,
        &data,
        file_name.as_deref(),
        now_ms(),
    )
    .map_err(BridgeError::storage)
}

#[tauri::command]
fn library_list(state: tauri::State<AppState>) -> Result<Vec<library::LibraryBook>, BridgeError> {
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    library::list_books(&db).map_err(|e| BridgeError::storage(e.to_string()))
}

#[tauri::command]
fn library_search(
    state: tauri::State<AppState>,
    query: String,
) -> Result<Vec<library::LibraryBook>, BridgeError> {
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    library::search_books(&db, &query).map_err(|e| BridgeError::storage(e.to_string()))
}

#[tauri::command]
fn library_source_records(
    state: tauri::State<AppState>,
    book_id: String,
) -> Result<Vec<library::LibrarySourceRecord>, BridgeError> {
    if book_id.trim().is_empty() {
        return Err(BridgeError::invalid_argument("bookId is required"));
    }
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    library::list_source_records(&db, &book_id).map_err(|e| BridgeError::storage(e.to_string()))
}

#[tauri::command]
fn library_link_remote_to_local(
    state: tauri::State<AppState>,
    remote_id: String,
    local_id: String,
) -> Result<library::LibraryBook, BridgeError> {
    if remote_id.trim().is_empty() || local_id.trim().is_empty() {
        return Err(BridgeError::invalid_argument(
            "remoteId and localId are required",
        ));
    }
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    library::link_remote_to_local(&db, &remote_id, &local_id, now_ms())
        .map_err(|e| BridgeError::storage(e.to_string()))
}

/// 在线元数据搜索（AniList）：拉索引/封面/简介 → 落库为远程条目（availability=remote）→
/// 返回新出现在书架上的条目。**只取元数据**，正文须经官方外链；HTTP 传输是壳的职责，
/// 解析/落库在 core。
#[tauri::command]
async fn library_search_remote(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<library::LibraryBook>, BridgeError> {
    search_remote_source(&state, "anilist", &query).await
}

/// 在线来源搜索：source=anilist/aozora。新增来源不复用旧消息塞隐式状态。
#[tauri::command]
async fn library_search_remote_source(
    state: tauri::State<'_, AppState>,
    source: String,
    query: String,
) -> Result<Vec<library::LibraryBook>, BridgeError> {
    search_remote_source(&state, &source, &query).await
}

async fn search_remote_source(
    state: &AppState,
    source: &str,
    query: &str,
) -> Result<Vec<library::LibraryBook>, BridgeError> {
    match source.trim().to_ascii_lowercase().as_str() {
        "anilist" => search_anilist(state, query).await,
        "bangumi" => search_bangumi(state, query).await,
        "aozora" => search_aozora(state, query).await,
        "narou" => search_narou(state, query).await,
        other => Err(BridgeError::invalid_argument(format!(
            "不支持的在线来源: {}",
            other
        ))),
    }
}

async fn search_anilist(
    state: &AppState,
    query: &str,
) -> Result<Vec<library::LibraryBook>, BridgeError> {
    use reading_core::connectors::{self, anilist};

    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    // 1) HTTP 传输（壳）。
    let body = anilist::search_request_body(q);
    let client = reqwest::Client::new();
    let resp = client
        .post(anilist::ENDPOINT)
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| BridgeError::network(format!("AniList 请求失败: {e}")))?;
    if !resp.status().is_success() {
        return Err(BridgeError::http_status_for(
            "AniList 请求失败",
            resp.status(),
        ));
    }
    let json = resp
        .text()
        .await
        .map_err(|e| BridgeError::network(format!("读取 AniList 响应失败: {e}")))?;

    // 2) 解析 + 落库（core）。
    let entries = anilist::parse_search(&json).map_err(BridgeError::parse)?;
    let now = now_ms();
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    connectors::ensure_source(
        &db,
        anilist::SOURCE_ID,
        anilist::SOURCE_NAME,
        "metadata",
        Some(anilist::ENDPOINT),
        now,
    )
    .map_err(|e| BridgeError::storage(e.to_string()))?;
    let ids = connectors::ingest(&db, anilist::SOURCE_ID, &entries, now)
        .map_err(|e| BridgeError::storage(e.to_string()))?;

    // 3) 回读落库后的条目返回前端。
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(b) =
            library::get_book(&db, &id).map_err(|e| BridgeError::storage(e.to_string()))?
        {
            out.push(b);
        }
    }
    Ok(out)
}

async fn search_bangumi(
    state: &AppState,
    query: &str,
) -> Result<Vec<library::LibraryBook>, BridgeError> {
    use reading_core::connectors::{self, bangumi};

    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    let body = bangumi::search_request_body(q);
    let client = reqwest::Client::new();
    let resp = client
        .post(bangumi::ENDPOINT)
        .query(&[("limit", BANGUMI_SEARCH_LIMIT.to_string())])
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .header(
            "user-agent",
            "LightNovel Reader/0.3.1 (https://github.com/haryqs/lightnovel-reader)",
        )
        .body(body)
        .send()
        .await
        .map_err(|e| BridgeError::network(format!("Bangumi request failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(BridgeError::http_status_for(
            "Bangumi request failed",
            resp.status(),
        ));
    }
    let json = resp
        .text()
        .await
        .map_err(|e| BridgeError::network(format!("read Bangumi response failed: {e}")))?;

    let entries = bangumi::parse_search(&json).map_err(BridgeError::parse)?;
    let now = now_ms();
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    connectors::ensure_source(
        &db,
        bangumi::SOURCE_ID,
        bangumi::SOURCE_NAME,
        "metadata",
        Some(bangumi::ENDPOINT),
        now,
    )
    .map_err(|e| BridgeError::storage(e.to_string()))?;
    let ids = connectors::ingest(&db, bangumi::SOURCE_ID, &entries, now)
        .map_err(|e| BridgeError::storage(e.to_string()))?;

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(b) =
            library::get_book(&db, &id).map_err(|e| BridgeError::storage(e.to_string()))?
        {
            out.push(b);
        }
    }
    Ok(out)
}

async fn search_narou(
    state: &AppState,
    query: &str,
) -> Result<Vec<library::LibraryBook>, BridgeError> {
    use reading_core::connectors::{self, narou};

    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    let params = [
        ("out", "json".to_string()),
        ("lim", NAROU_SEARCH_LIMIT.to_string()),
        ("word", q.to_string()),
        ("order", "hyoka".to_string()),
        // ncode/title/writer/story are enough for the first metadata pass.
        ("of", "n-t-w-s".to_string()),
    ];
    let client = reqwest::Client::new();
    let resp = client
        .get(narou::ENDPOINT)
        .query(&params)
        .header("accept", "application/json")
        .header("user-agent", "LightNovel Reader")
        .send()
        .await
        .map_err(|e| BridgeError::network(format!("Narou request failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(BridgeError::http_status_for(
            "Narou request failed",
            resp.status(),
        ));
    }
    let json = resp
        .text()
        .await
        .map_err(|e| BridgeError::network(format!("read Narou response failed: {e}")))?;

    let entries = narou::parse_search(&json).map_err(BridgeError::parse)?;
    let now = now_ms();
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    connectors::ensure_source(
        &db,
        narou::SOURCE_ID,
        narou::SOURCE_NAME,
        "metadata",
        Some(narou::ENDPOINT),
        now,
    )
    .map_err(|e| BridgeError::storage(e.to_string()))?;
    let ids = connectors::ingest(&db, narou::SOURCE_ID, &entries, now)
        .map_err(|e| BridgeError::storage(e.to_string()))?;

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(b) =
            library::get_book(&db, &id).map_err(|e| BridgeError::storage(e.to_string()))?
        {
            out.push(b);
        }
    }
    Ok(out)
}

async fn search_aozora(
    state: &AppState,
    query: &str,
) -> Result<Vec<library::LibraryBook>, BridgeError> {
    use reading_core::connectors::{self, aozora};

    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    let csv = load_aozora_catalog_csv(&state.cache_dir).await?;
    let entries =
        aozora::parse_catalog_csv(&csv, q, AOZORA_SEARCH_LIMIT).map_err(BridgeError::parse)?;
    let now = now_ms();
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    connectors::ensure_source(
        &db,
        aozora::SOURCE_ID,
        aozora::SOURCE_NAME,
        "catalog",
        Some(aozora::CATALOG_ZIP_URL),
        now,
    )
    .map_err(|e| BridgeError::storage(e.to_string()))?;
    let ids = connectors::ingest(&db, aozora::SOURCE_ID, &entries, now)
        .map_err(|e| BridgeError::storage(e.to_string()))?;

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(b) =
            library::get_book(&db, &id).map_err(|e| BridgeError::storage(e.to_string()))?
        {
            out.push(b);
        }
    }
    Ok(out)
}

#[tauri::command]
async fn library_acquire_remote(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<library::LibraryBook, BridgeError> {
    use reading_core::connectors::aozora;

    if id.trim().is_empty() {
        return Err(BridgeError::invalid_argument("remote id is required"));
    }

    let acquisition = {
        let db = state
            .library_db
            .lock()
            .map_err(|e| BridgeError::storage(e.to_string()))?;
        let Some(info) = library::remote_acquisition(&db, &id)
            .map_err(|e| BridgeError::storage(e.to_string()))?
        else {
            return Err(BridgeError::not_found("找不到可获取的远程条目"));
        };
        info
    };

    if acquisition.source_id != aozora::SOURCE_ID {
        return Err(BridgeError::forbidden("当前只支持获取青空文库公共版权条目"));
    }
    if acquisition.rights_status != "public_domain" {
        return Err(BridgeError::forbidden(
            "该条目不是公共版权，不能下载正文；请跳转官方链接",
        ));
    }
    if let Some(existing) = acquisition.existing_asset_id.as_deref() {
        let db = state
            .library_db
            .lock()
            .map_err(|e| BridgeError::storage(e.to_string()))?;
        if let Some(book) =
            library::get_book(&db, existing).map_err(|e| BridgeError::storage(e.to_string()))?
        {
            return Ok(book);
        }
    }

    let csv = load_aozora_catalog_csv(&state.cache_dir).await?;
    let work = aozora::find_catalog_work_by_id(&csv, &acquisition.remote_id)
        .map_err(BridgeError::parse)?
        .ok_or_else(|| BridgeError::not_found("青空目录中找不到该作品"))?;
    if work.rights_status != "public_domain" {
        return Err(BridgeError::forbidden(
            "青空目录显示该作品非公共版权，不能下载正文",
        ));
    }
    let html_url = work.html_url.as_deref().ok_or_else(|| {
        BridgeError::not_found("该青空条目没有 XHTML/HTML 正文 URL，暂不能站内阅读")
    })?;
    ensure_aozora_url(html_url)?;
    let html = fetch_text(html_url, "青空正文").await?;

    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    library::attach_remote_html_asset(
        &db,
        &state.library_dir,
        &acquisition.edition_id,
        &acquisition.title,
        acquisition.author.as_deref(),
        acquisition.language.as_deref(),
        html_url,
        &html,
        now_ms(),
    )
    .map_err(BridgeError::storage)
}

async fn load_aozora_catalog_csv(cache_dir: &Path) -> Result<String, BridgeError> {
    use reading_core::connectors::aozora;

    let dir = cache_dir.join("connectors").join("aozora");
    let csv_path = dir.join("list_person_all_extended_utf8.csv");
    if catalog_cache_is_fresh(&csv_path) {
        return std::fs::read_to_string(&csv_path)
            .map_err(|e| BridgeError::storage(format!("读取青空目录缓存失败: {e}")));
    }

    let bytes = fetch_bytes(aozora::CATALOG_ZIP_URL, "青空目录").await?;
    let csv = tauri::async_runtime::spawn_blocking(move || extract_csv_from_zip(&bytes))
        .await
        .map_err(|e| BridgeError::storage(format!("解压青空目录任务失败: {e}")))??;
    std::fs::create_dir_all(&dir)
        .map_err(|e| BridgeError::storage(format!("创建青空缓存目录失败: {e}")))?;
    let tmp = csv_path.with_extension("csv.tmp");
    std::fs::write(&tmp, csv.as_bytes())
        .map_err(|e| BridgeError::storage(format!("写入青空目录缓存失败: {e}")))?;
    std::fs::rename(&tmp, &csv_path)
        .map_err(|e| BridgeError::storage(format!("更新青空目录缓存失败: {e}")))?;
    Ok(csv)
}

fn catalog_cache_is_fresh(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    modified
        .elapsed()
        .is_ok_and(|age| age <= AOZORA_CATALOG_MAX_AGE)
}

fn extract_csv_from_zip(bytes: &[u8]) -> Result<String, BridgeError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| BridgeError::parse(format!("打开青空目录 ZIP 失败: {e}")))?;
    let mut csv_name = None;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| BridgeError::parse(e.to_string()))?;
        if entry.name().to_ascii_lowercase().ends_with(".csv") {
            csv_name = Some(entry.name().to_string());
            break;
        }
    }
    let name = csv_name.ok_or_else(|| BridgeError::parse("青空目录 ZIP 中未找到 CSV"))?;
    let mut entry = archive
        .by_name(&name)
        .map_err(|e| BridgeError::parse(format!("读取青空目录 CSV 失败: {e}")))?;
    let mut buf = Vec::new();
    entry
        .read_to_end(&mut buf)
        .map_err(|e| BridgeError::parse(format!("读取青空目录 CSV 字节失败: {e}")))?;
    String::from_utf8(buf).map_err(|e| BridgeError::parse(format!("青空目录不是 UTF-8: {e}")))
}

async fn fetch_bytes(url: &str, label: &str) -> Result<Vec<u8>, BridgeError> {
    let resp = reqwest::Client::new()
        .get(url)
        .header("user-agent", "LightNovel Reader/0.3.1")
        .send()
        .await
        .map_err(|e| BridgeError::network(format!("下载{label}失败: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(BridgeError::http_status_for(
            format!("下载{label}失败"),
            status,
        ));
    }
    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| BridgeError::network(format!("读取{label}响应失败: {e}")))
}

async fn fetch_text(url: &str, label: &str) -> Result<String, BridgeError> {
    let resp = reqwest::Client::new()
        .get(url)
        .header("user-agent", "LightNovel Reader/0.3.1")
        .send()
        .await
        .map_err(|e| BridgeError::network(format!("下载{label}失败: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(BridgeError::http_status_for(
            format!("下载{label}失败"),
            status,
        ));
    }
    resp.text()
        .await
        .map_err(|e| BridgeError::network(format!("读取{label}响应失败: {e}")))
}

fn ensure_aozora_url(url: &str) -> Result<(), BridgeError> {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("https://www.aozora.gr.jp/")
        || lower.starts_with("http://www.aozora.gr.jp/")
    {
        Ok(())
    } else {
        Err(BridgeError::forbidden(
            "青空正文 URL 不属于官方 aozora.gr.jp，已拒绝下载",
        ))
    }
}

#[tauri::command]
async fn library_open(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<OpenedBook, BridgeError> {
    if id.trim().is_empty() {
        return Err(BridgeError::invalid_argument("book id is required"));
    }
    let file_path = {
        let db = state
            .library_db
            .lock()
            .map_err(|e| BridgeError::storage(e.to_string()))?;
        let book = library::get_book(&db, &id)
            .map_err(|e| BridgeError::storage(e.to_string()))?
            .ok_or_else(|| BridgeError::not_found("书库中找不到这本书"))?;
        // 远程 metadata_only 条目无本地文件，不能站内打开（应由前端走外链）。
        book.file_path.ok_or_else(|| {
            BridgeError::not_found("该条目没有本地文件，无法打开（远程条目请用外部链接）")
        })?
    };
    let data = std::fs::read(&file_path)
        .map_err(|e| BridgeError::storage(format!("读取书库文件失败: {}", e)))?;
    load_book_from_data(&state, data).map_err(BridgeError::parse)
}

#[tauri::command]
fn library_touch_last_read(state: tauri::State<AppState>, id: String) -> Result<(), BridgeError> {
    if id.trim().is_empty() {
        return Err(BridgeError::invalid_argument("book id is required"));
    }
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    library::touch_last_read(&db, &id, now_ms()).map_err(|e| BridgeError::storage(e.to_string()))
}

#[tauri::command]
fn close_book(state: tauri::State<AppState>) -> Result<(), String> {
    let mut book = state.book.lock().map_err(|e| e.to_string())?;
    *book = None;
    Ok(())
}

// —— 标注持久化命令 ——

#[tauri::command]
fn save_annotation(
    state: tauri::State<AppState>,
    annotation: storage::Annotation,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    storage::save(&db, &annotation).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_annotations(
    state: tauri::State<AppState>,
    book_id: String,
) -> Result<Vec<storage::Annotation>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    storage::list(&db, &book_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_annotation(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    storage::delete(&db, &id).map_err(|e| e.to_string())
}

// —— 阅读进度命令 ——

#[tauri::command]
fn save_progress(
    state: tauri::State<AppState>,
    progress: storage::ReadingProgress,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    storage::save_progress(&db, &progress).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_progress(
    state: tauri::State<AppState>,
    book_id: String,
) -> Result<Option<storage::ReadingProgress>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    storage::get_progress(&db, &book_id).map_err(|e| e.to_string())
}

// ── OPDS v0.6 命令 ──

/// 添加一个 OPDS 书源（kind="opds"），幂等。
#[tauri::command]
async fn opds_add_source(
    state: tauri::State<'_, AppState>,
    name: String,
    url: String,
) -> Result<connectors::opds::OpdsSource, BridgeError> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let url = url.trim().to_string();
    if url.is_empty() {
        return Err(BridgeError::invalid_argument("URL is required"));
    }

    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let source_id = format!("opds-{:x}", hasher.finish());

    let now = now_ms();
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    connectors::ensure_source(&db, &source_id, &name, "opds", Some(&url), now)
        .map_err(|e| BridgeError::storage(e.to_string()))?;

    Ok(connectors::opds::OpdsSource {
        id: source_id,
        name,
        base_url: Some(url),
        enabled: true,
    })
}

/// 移除一个 OPDS 书源。
#[tauri::command]
async fn opds_remove_source(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), BridgeError> {
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    connectors::opds::remove_source(&db, &id).map_err(|e| BridgeError::storage(e.to_string()))
}

/// 列出所有已添加的 OPDS 书源。
#[tauri::command]
async fn opds_list_sources(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<connectors::opds::OpdsSource>, BridgeError> {
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    connectors::opds::list_sources(&db).map_err(|e| BridgeError::storage(e.to_string()))
}

/// 抓取并解析一个 OPDS feed（导航或获取），不做落库。
/// 自动检测 OPDS 1.x (Atom XML) 与 OPDS 2.0 (JSON) 格式。
#[tauri::command]
async fn opds_browse_feed(url: String) -> Result<connectors::opds::OpdsFeed, BridgeError> {
    let url = url.trim();
    if url.is_empty() {
        return Err(BridgeError::invalid_argument("URL is required"));
    }

    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header("accept", "application/atom+xml, application/opds+json, application/xml, text/xml, application/json")
        .header(
            "user-agent",
            "LightNovel-Reader/0.6 (OPDS client; https://github.com/haryqs/lightnovel-reader)",
        )
        .send()
        .await
        .map_err(|e| BridgeError::network(format!("OPDS request failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(BridgeError::http_status(resp.status()));
    }

    let body = resp
        .text()
        .await
        .map_err(|e| BridgeError::network(format!("read response: {e}")))?;
    opds_parse_body(&body)
}

/// Auto-detect OPDS format (1.x XML vs 2.0 JSON) and parse.
fn opds_parse_body(body: &str) -> Result<connectors::opds::OpdsFeed, BridgeError> {
    let trimmed = body.trim_start();
    if trimmed.starts_with('{') {
        connectors::opds::parse_opds_2x(body).map_err(BridgeError::parse)
    } else {
        connectors::opds::parse_opds_1x(body).map_err(BridgeError::parse)
    }
}

/// 在已添加的 OPDS 书源中搜索。
#[tauri::command]
async fn opds_search_feed(
    state: tauri::State<'_, AppState>,
    source_id: String,
    query: String,
) -> Result<connectors::opds::OpdsFeed, BridgeError> {
    let q = query.trim();
    if q.is_empty() {
        return Err(BridgeError::invalid_argument("search query is empty"));
    }

    // 在 await 之前取出 base_url 并释放 db 锁（MutexGuard 不能跨 await）。
    let base_url: String = {
        let db = state
            .library_db
            .lock()
            .map_err(|e| BridgeError::storage(e.to_string()))?;
        db.query_row(
            "SELECT base_url FROM source WHERE id = ?1 AND kind = 'opds'",
            [&source_id],
            |row| row.get(0),
        )
        .map_err(|e| BridgeError::not_found(format!("source not found: {e}")))?
    };

    let search_url = connectors::opds::search_url(&base_url, q);

    let client = reqwest::Client::new();
    let resp = client
        .get(&search_url)
        .header("accept", "application/atom+xml, application/opds+json, application/xml, text/xml, application/json")
        .header(
            "user-agent",
            "LightNovel-Reader/0.6 (OPDS client; https://github.com/haryqs/lightnovel-reader)",
        )
        .send()
        .await
        .map_err(|e| BridgeError::network(format!("OPDS search request failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(BridgeError::http_status(resp.status()));
    }

    let body = resp
        .text()
        .await
        .map_err(|e| BridgeError::network(format!("read search response: {e}")))?;
    opds_parse_body(&body)
}

/// 把 OPDS feed 中的条目落库为远程书库条目（仅元数据，不拉正文）。
#[tauri::command]
async fn opds_ingest_entries(
    state: tauri::State<'_, AppState>,
    source_id: String,
    feed: connectors::opds::OpdsFeed,
) -> Result<Vec<library::LibraryBook>, BridgeError> {
    let now = now_ms();
    let entries: Vec<connectors::RemoteEntry> = feed
        .entries
        .iter()
        .filter(|e| !e.is_navigation)
        .map(|e| e.to_remote_entry(&source_id))
        .collect();

    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    // 确保来源存在（幂等）
    connectors::ensure_source(&db, &source_id, &source_id, "opds", None, now)
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    let ids = connectors::ingest(&db, &source_id, &entries, now)
        .map_err(|e| BridgeError::storage(e.to_string()))?;

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(b) =
            library::get_book(&db, &id).map_err(|e| BridgeError::storage(e.to_string()))?
        {
            out.push(b);
        }
    }
    Ok(out)
}

/// 下载 OPDS open_license EPUB 并转为本地可读资产。
/// 只允许 rights_status=open_license 的条目。
#[tauri::command]
async fn opds_download_epub(
    state: tauri::State<'_, AppState>,
    edition_id: String,
    acquisition_url: String,
) -> Result<library::LibraryBook, BridgeError> {
    // 1) 验证条目存在且为 open_license。
    let acquisition = {
        let db = state
            .library_db
            .lock()
            .map_err(|e| BridgeError::storage(e.to_string()))?;
        let Some(info) = library::remote_acquisition(&db, &edition_id)
            .map_err(|e| BridgeError::storage(e.to_string()))?
        else {
            return Err(BridgeError::not_found("找不到该条目"));
        };
        if info.rights_status != "open_license" {
            return Err(BridgeError::forbidden(format!(
                "该条目授权状态为 {}，不支持下载正文",
                info.rights_status
            )));
        }
        info
    };

    // 2) 下载 EPUB 字节。
    let client = reqwest::Client::new();
    let resp = client
        .get(&acquisition_url)
        .header(
            "user-agent",
            "LightNovel-Reader/0.6 (OPDS client; https://github.com/haryqs/lightnovel-reader)",
        )
        .send()
        .await
        .map_err(|e| BridgeError::network(format!("下载 EPUB 失败: {e}")))?;
    if !resp.status().is_success() {
        return Err(BridgeError::http_status(resp.status()));
    }
    let epub_bytes = resp
        .bytes()
        .await
        .map_err(|e| BridgeError::network(format!("读取 EPUB 响应失败: {e}")))?;

    // 3) 落库（core）。
    let now = now_ms();
    let db = state
        .library_db
        .lock()
        .map_err(|e| BridgeError::storage(e.to_string()))?;
    library::attach_remote_epub_bytes(
        &db,
        &state.library_dir,
        &acquisition.edition_id,
        &epub_bytes,
        now,
    )
    .map_err(BridgeError::storage)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // 图片协议：正文里的 <img src="reader-img://localhost/<路径>"> 由此按需从书内读取，
        // 既能显示封面/插图，又不把图片 base64 内联进 HTML。
        .register_uri_scheme_protocol("reader-img", |ctx, request| {
            let app = ctx.app_handle();
            let path = percent_decode(request.uri().path().trim_start_matches('/'));
            eprintln!("reader-img request: {}", path);
            let bytes_opt = {
                let state = app.state::<AppState>();
                let guard = state.book.lock().ok();
                guard
                    .as_ref()
                    .and_then(|g| g.as_ref())
                    .map(|b| b.bytes.clone())
            };
            if let Some(bytes) = bytes_opt {
                if let Some((mime, data)) = epub_parser::read_image_from_zip(&bytes[..], &path) {
                    eprintln!("reader-img hit: {} ({} bytes)", path, data.len());
                    return tauri::http::Response::builder()
                        .status(200)
                        .header("Content-Type", mime)
                        .body(Cow::Owned(data))
                        .unwrap();
                }
            }
            tauri::http::Response::builder()
                .status(404)
                .body(Cow::Owned(Vec::new()))
                .unwrap()
        })
        .setup(|app| {
            let dir = resolve_app_data_dir(app);
            std::fs::create_dir_all(&dir).ok();
            let conn = storage::init(&dir.join("reader.db")).expect("SQLite 初始化失败");
            let library_dir = dir.join("library");
            std::fs::create_dir_all(&library_dir).expect("书库目录初始化失败");
            let library_conn = library::open_library(&library_dir.join("library.sqlite"))
                .expect("书库 SQLite 初始化失败");
            let cache_dir = dir.join("cache");
            std::fs::create_dir_all(&cache_dir).ok();
            app.manage(AppState {
                book: Mutex::new(None),
                db: Mutex::new(conn),
                library_db: Mutex::new(library_conn),
                library_dir,
                cache_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_book_bytes,
            open_book_path,
            list_calibre_books,
            library_import,
            library_import_bytes,
            library_list,
            library_search,
            library_source_records,
            library_link_remote_to_local,
            library_search_remote,
            library_search_remote_source,
            library_acquire_remote,
            library_open,
            library_touch_last_read,
            get_chapter,
            close_book,
            save_annotation,
            list_annotations,
            delete_annotation,
            save_progress,
            get_progress,
            opds_add_source,
            opds_remove_source,
            opds_list_sources,
            opds_browse_feed,
            opds_search_feed,
            opds_ingest_entries,
            opds_download_epub,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
