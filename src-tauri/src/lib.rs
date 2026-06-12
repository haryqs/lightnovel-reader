use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use reading_core::epub_parser::{self, BookInfo};
use reading_core::{compute_book_id, library, rusqlite, storage};
use tauri::Manager;

struct LoadedBook {
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
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn load_book_from_data(state: &AppState, data: Vec<u8>) -> Result<OpenedBook, String> {
    let book_id = compute_book_id(&data);
    let book_info = epub_parser::parse_book_info(&data)?;
    let mut chapters = HashMap::new();
    if let Some(first) = book_info.spine.first() {
        let href = first.href.clone();
        if let Ok(html) = epub_parser::parse_single_chapter(&data, &href, &book_info) {
            chapters.insert(href, html);
        }
    }
    let loaded = LoadedBook {
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

    // 缓存未命中——按需解析
    eprintln!("  缓存未命中，按需解析");
    let html = epub_parser::parse_single_chapter(&book.bytes[..], &href, &book.book_info)?;

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
async fn list_calibre_books(library: String) -> Result<Vec<CalibreBook>, String> {
    let db_path = std::path::Path::new(&library).join("metadata.db");
    let conn =
        rusqlite::Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("打开 Calibre 库失败: {}", e))?;

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
        .map_err(|e| e.to_string())?;

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
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

// —— 自有书库命令 ——

#[tauri::command]
fn library_import(
    state: tauri::State<AppState>,
    path: String,
) -> Result<library::ImportOutcome, String> {
    let db = state.library_db.lock().map_err(|e| e.to_string())?;
    library::import_epub(&db, &state.library_dir, Path::new(&path), now_ms())
}

#[tauri::command]
fn library_list(state: tauri::State<AppState>) -> Result<Vec<library::LibraryBook>, String> {
    let db = state.library_db.lock().map_err(|e| e.to_string())?;
    library::list_books(&db).map_err(|e| e.to_string())
}

#[tauri::command]
fn library_search(
    state: tauri::State<AppState>,
    query: String,
) -> Result<Vec<library::LibraryBook>, String> {
    let db = state.library_db.lock().map_err(|e| e.to_string())?;
    library::search_books(&db, &query).map_err(|e| e.to_string())
}

#[tauri::command]
async fn library_open(state: tauri::State<'_, AppState>, id: String) -> Result<OpenedBook, String> {
    let file_path = {
        let db = state.library_db.lock().map_err(|e| e.to_string())?;
        let book = library::get_book(&db, &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "书库中找不到这本书".to_string())?;
        book.file_path
    };
    let data = std::fs::read(&file_path).map_err(|e| format!("读取书库文件失败: {}", e))?;
    load_book_from_data(&state, data)
}

#[tauri::command]
fn library_touch_last_read(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let db = state.library_db.lock().map_err(|e| e.to_string())?;
    library::touch_last_read(&db, &id, now_ms()).map_err(|e| e.to_string())
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // 图片协议：正文里的 <img src="reader-img://localhost/<路径>"> 由此按需从书内读取，
        // 既能显示封面/插图，又不把图片 base64 内联进 HTML。
        .register_uri_scheme_protocol("reader-img", |ctx, request| {
            let app = ctx.app_handle();
            let path = percent_decode(request.uri().path().trim_start_matches('/'));
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
            let dir = app.path().app_data_dir().expect("无法解析 app data 目录");
            std::fs::create_dir_all(&dir).ok();
            let conn = storage::init(&dir.join("reader.db")).expect("SQLite 初始化失败");
            let library_dir = dir.join("library");
            std::fs::create_dir_all(&library_dir).expect("书库目录初始化失败");
            let library_conn = library::open_library(&library_dir.join("library.sqlite"))
                .expect("书库 SQLite 初始化失败");
            app.manage(AppState {
                book: Mutex::new(None),
                db: Mutex::new(conn),
                library_db: Mutex::new(library_conn),
                library_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_book_bytes,
            open_book_path,
            list_calibre_books,
            library_import,
            library_list,
            library_search,
            library_open,
            library_touch_last_read,
            get_chapter,
            close_book,
            save_annotation,
            list_annotations,
            delete_annotation,
            save_progress,
            get_progress,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
