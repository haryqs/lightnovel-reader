//! 元数据连接器：从合法的开放/官方元数据源拉取**索引**（标题/封面/简介/系列），
//! 写成 `series/volume/edition` + `source_record`（`availability=remote`）。
//!
//! 严格边界（PROJECT_MEMORY 版权红线）：
//! - **只拉元数据，不拉正文**。封面/简介按来源 API 条款使用，封面以 URL 引用、不再托管。
//! - 能不能站内阅读由 `availability` 决定：远程条目恒 `remote` → 前端只展示 + 跳官方外链。
//! - 不爬商业站正文、不聚合盗版源。连接器只对接有正规接口与条款的元数据提供方。
//!
//! 分层：本模块只做「查询构造 + 响应解析 + 落库」（纯函数，可单测/可 wasm）；
//! 真正的 HTTP 传输是平台胶水，放在各壳（Tauri command 等）。

use rusqlite::{params, Connection};

/// 从某来源解析出的一条远程条目（尚未落库的中间形态）。
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteEntry {
    /// 来源内的稳定 id（如 AniList media id），用于派生确定性实体 id、可重入。
    pub remote_id: String,
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
    /// 封面 URL（按来源条款引用，不再托管到本地对象仓库）。
    pub cover_url: Option<String>,
    /// 语种码（ja/zh/ko/en…）。
    pub language: Option<String>,
    /// 官方/来源页外链——受版权条目点击后跳这里。
    pub site_url: Option<String>,
    /// 授权状态（public_domain/open_license/official_free/official_purchase/unknown…）。
    pub rights_status: String,
}

/// 注册/更新一个来源行（幂等）。
pub fn ensure_source(
    conn: &Connection,
    id: &str,
    name: &str,
    kind: &str,
    base_url: Option<&str>,
    now_ms: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO source(id, name, kind, base_url, enabled, created_at)
         VALUES (?1, ?2, ?3, ?4, 1, ?5)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, base_url = excluded.base_url",
        params![id, name, kind, base_url, now_ms],
    )?;
    Ok(())
}

/// 把一批远程条目落库为 series←volume←edition + source_record（availability=remote，无 asset）。
/// 用 `source_id + remote_id` 派生确定性 id → 重复搜索幂等刷新，不产生重复条目。
/// 返回各条目的 edition id（= 书架条目 id，可据此 `library::get_book` 取回）。
pub fn ingest(
    conn: &Connection,
    source_id: &str,
    entries: &[RemoteEntry],
    now_ms: i64,
) -> rusqlite::Result<Vec<String>> {
    let mut edition_ids = Vec::with_capacity(entries.len());
    for e in entries {
        // 一个来源条目 → 1 series + 1 volume + 1 edition（AniList 不细分卷）。
        let series_id = format!("series:{}:{}", source_id, e.remote_id);
        let volume_id = format!("vol:{}:{}", source_id, e.remote_id);
        let edition_id = format!("ed:{}:{}", source_id, e.remote_id);
        let record_id = format!("sr:{}:{}", source_id, e.remote_id);

        conn.execute(
            "INSERT INTO series(id, title, author, description, cover_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(id) DO UPDATE SET
               title = excluded.title, author = excluded.author,
               description = excluded.description, cover_path = excluded.cover_path,
               updated_at = excluded.updated_at",
            params![series_id, e.title, e.author, e.description, e.cover_url, now_ms],
        )?;
        conn.execute(
            "INSERT INTO volume(id, series_id, kind, volume_number, title, description, created_at, updated_at)
             VALUES (?1, ?2, 'main', NULL, ?3, ?4, ?5, ?5)
             ON CONFLICT(id) DO UPDATE SET
               title = excluded.title, description = excluded.description, updated_at = excluded.updated_at",
            params![volume_id, series_id, e.title, e.description, now_ms],
        )?;
        conn.execute(
            "INSERT INTO edition(id, volume_id, language, rights_status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(id) DO UPDATE SET
               language = excluded.language, rights_status = excluded.rights_status,
               updated_at = excluded.updated_at",
            params![edition_id, volume_id, e.language, e.rights_status, now_ms],
        )?;
        conn.execute(
            "INSERT INTO source_record(id, source_id, entity_type, entity_id, remote_url, remote_id, rights_status, availability, last_checked_at)
             VALUES (?1, ?2, 'edition', ?3, ?4, ?5, ?6, 'remote', ?7)
             ON CONFLICT(id) DO UPDATE SET
               remote_url = excluded.remote_url, rights_status = excluded.rights_status,
               availability = excluded.availability, last_checked_at = excluded.last_checked_at",
            params![record_id, source_id, edition_id, e.site_url, e.remote_id, e.rights_status, now_ms],
        )?;

        edition_ids.push(edition_id);
    }
    Ok(edition_ids)
}

/// AniList 连接器：轻小说/ACG 元数据，公开 GraphQL API（无需鉴权），只读元数据。
pub mod anilist {
    use super::RemoteEntry;
    use serde::Deserialize;

    pub const SOURCE_ID: &str = "src:anilist";
    pub const SOURCE_NAME: &str = "AniList";
    pub const ENDPOINT: &str = "https://graphql.anilist.co";

    /// 构造搜索请求体（JSON）。type=MANGA + format=NOVEL 即轻小说。
    /// 调用方把它作为 `application/json` POST 到 [`ENDPOINT`]。
    pub fn search_request_body(term: &str) -> String {
        const QUERY: &str = "query ($search: String) { \
            Page(perPage: 20) { media(search: $search, type: MANGA, format: NOVEL) { \
              id title { romaji english native } description(asHtml: false) \
              coverImage { large } countryOfOrigin siteUrl \
              staff(perPage: 1) { nodes { name { full } } } } } }";
        let body = serde_json::json!({ "query": QUERY, "variables": { "search": term } });
        body.to_string()
    }

    #[derive(Deserialize)]
    struct Resp {
        data: Option<RespData>,
    }
    #[derive(Deserialize)]
    struct RespData {
        #[serde(rename = "Page")]
        page: Option<Page>,
    }
    #[derive(Deserialize)]
    struct Page {
        media: Option<Vec<Media>>,
    }
    #[derive(Deserialize)]
    struct Media {
        id: i64,
        title: Option<Title>,
        description: Option<String>,
        #[serde(rename = "coverImage")]
        cover_image: Option<CoverImage>,
        #[serde(rename = "countryOfOrigin")]
        country_of_origin: Option<String>,
        #[serde(rename = "siteUrl")]
        site_url: Option<String>,
        staff: Option<Staff>,
    }
    #[derive(Deserialize)]
    struct Title {
        romaji: Option<String>,
        english: Option<String>,
        native: Option<String>,
    }
    #[derive(Deserialize)]
    struct CoverImage {
        large: Option<String>,
    }
    #[derive(Deserialize)]
    struct Staff {
        nodes: Option<Vec<StaffNode>>,
    }
    #[derive(Deserialize)]
    struct StaffNode {
        name: Option<StaffName>,
    }
    #[derive(Deserialize)]
    struct StaffName {
        full: Option<String>,
    }

    /// ISO 国家码 → 语种码（AniList 给的是来源国，足够标注语种）。
    fn country_to_lang(c: &str) -> Option<String> {
        match c {
            "JP" => Some("ja".into()),
            "KR" => Some("ko".into()),
            "CN" | "TW" => Some("zh".into()),
            _ => None,
        }
    }

    /// 解析搜索响应为 [`RemoteEntry`]。容忍缺字段；无标题的条目跳过。
    pub fn parse_search(json: &str) -> Result<Vec<RemoteEntry>, String> {
        let resp: Resp = serde_json::from_str(json).map_err(|e| format!("解析 AniList 响应失败: {e}"))?;
        let media = resp
            .data
            .and_then(|d| d.page)
            .and_then(|p| p.media)
            .unwrap_or_default();

        let mut out = Vec::new();
        for m in media {
            let title = m.title.and_then(|t| t.romaji.or(t.english).or(t.native));
            let Some(title) = title.filter(|s| !s.trim().is_empty()) else {
                continue;
            };
            let author = m
                .staff
                .and_then(|s| s.nodes)
                .and_then(|mut n| n.drain(..).next())
                .and_then(|node| node.name)
                .and_then(|name| name.full);
            out.push(RemoteEntry {
                remote_id: m.id.to_string(),
                title,
                author,
                description: m.description.filter(|s| !s.trim().is_empty()),
                cover_url: m.cover_image.and_then(|c| c.large),
                language: m.country_of_origin.as_deref().and_then(country_to_lang),
                site_url: m.site_url,
                // AniList 收录的是商业出版物 → 默认须官方购买；站内只展示 + 外链。
                rights_status: "official_purchase".into(),
            });
        }
        Ok(out)
    }
}

/// 青空文库连接器：日本公共版权文学。来源 = 官方「全作品扩展目录」CSV（UTF-8，打包为 zip）。
/// 只对接官方权威数据（不依赖第三方服务），ToS 干净；正文亦在官方站，留待 PR-B 做站内阅览。
///
/// 与 AniList 不同：青空作品多为 `著作権フラグ=なし`（公共版权）→ rights 给 `public_domain`，
/// 是首个未来可"站内自由阅览"的真实来源。本模块只做 CSV 解析（纯函数，可测/可 wasm）；
/// CSV 的下载 + 解压是壳的职责（目录较大，壳侧应缓存复用）。
pub mod aozora {
    use super::RemoteEntry;
    use std::collections::HashSet;

    pub const SOURCE_ID: &str = "src:aozora";
    pub const SOURCE_NAME: &str = "青空文庫";
    /// 官方全作品扩展目录（UTF-8 CSV，zip 打包）。壳下载并解压后把 CSV 文本交给 [`parse_catalog_csv`]。
    pub const CATALOG_ZIP_URL: &str =
        "https://www.aozora.gr.jp/index_pages/list_person_all_extended_utf8.zip";

    /// 解析官方扩展目录 CSV，按「作品名包含 query」过滤，映射为 [`RemoteEntry`]。
    ///
    /// - **按表头名取列**（作品ID/作品名/作品著作権フラグ/図書カードURL/姓/名），抗列序变化；
    ///   缺必需列（作品ID/作品名）即报错（目录格式变了，早失败）。
    /// - 一个作品在扩展目录里可能多行（著者/翻译者各一行）→ 按作品ID去重，保留首行。
    /// - `著作権フラグ=なし` → `public_domain`；否则 `unknown`。青空作品无封面图、目录无简介。
    /// - `limit` 截断（目录上万行）。
    pub fn parse_catalog_csv(csv_text: &str, query: &str, limit: usize) -> Result<Vec<RemoteEntry>, String> {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(csv_text.as_bytes());
        let headers = rdr
            .headers()
            .map_err(|e| format!("青空目录表头解析失败: {e}"))?
            .clone();
        let col = |name: &str| headers.iter().position(|h| h == name);
        let c_id = col("作品ID").ok_or("青空目录缺『作品ID』列")?;
        let c_title = col("作品名").ok_or("青空目录缺『作品名』列")?;
        let c_copyright = col("作品著作権フラグ");
        let c_card = col("図書カードURL");
        let c_last = col("姓");
        let c_first = col("名");

        let q = query.trim().to_lowercase();
        let mut seen: HashSet<String> = HashSet::new();
        let mut out = Vec::new();

        for rec in rdr.records() {
            let rec = rec.map_err(|e| format!("青空目录行解析失败: {e}"))?;
            let get = |i: Option<usize>| {
                i.and_then(|i| rec.get(i))
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            };

            let Some(title) = get(Some(c_title)) else { continue };
            if !q.is_empty() && !title.to_lowercase().contains(&q) {
                continue;
            }
            let Some(remote_id) = get(Some(c_id)) else { continue };
            if !seen.insert(remote_id.to_string()) {
                continue; // 同作品多行去重
            }

            let rights_status = match get(c_copyright) {
                Some("なし") => "public_domain",
                _ => "unknown",
            }
            .to_string();
            let author = match (get(c_last), get(c_first)) {
                (Some(l), Some(f)) => Some(format!("{}{}", l, f)), // 姓名相连（日文无空格）
                (Some(l), None) => Some(l.to_string()),
                (None, Some(f)) => Some(f.to_string()),
                _ => None,
            };

            out.push(RemoteEntry {
                remote_id: remote_id.to_string(),
                title: title.to_string(),
                author,
                description: None,
                cover_url: None,
                language: Some("ja".into()),
                site_url: get(c_card).map(str::to_string),
                rights_status,
            });
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library;

    fn open_db() -> (std::path::PathBuf, Connection) {
        let dir = std::env::temp_dir().join(format!("reading-core-conntest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let conn = library::open_library(&dir.join("library.sqlite")).unwrap();
        (dir, conn)
    }

    const FIXTURE: &str = r#"{
      "data": { "Page": { "media": [
        {
          "id": 98329,
          "title": { "romaji": "Youjo Senki", "english": "The Saga of Tanya the Evil", "native": "幼女戦記" },
          "description": "A reincarnated salaryman as a little girl on the battlefield.",
          "coverImage": { "large": "https://img.anili.st/98329.jpg" },
          "countryOfOrigin": "JP",
          "siteUrl": "https://anilist.co/manga/98329",
          "staff": { "nodes": [ { "name": { "full": "Carlo Zen" } } ] }
        },
        {
          "id": 86411,
          "title": { "romaji": null, "english": null, "native": "本好きの下剋上" },
          "description": null,
          "coverImage": { "large": null },
          "countryOfOrigin": "JP",
          "siteUrl": "https://anilist.co/manga/86411",
          "staff": { "nodes": [] }
        }
      ] } }
    }"#;

    #[test]
    fn parse_search_maps_fields_and_skips_titleless() {
        let entries = anilist::parse_search(FIXTURE).unwrap();
        assert_eq!(entries.len(), 2);

        let a = &entries[0];
        assert_eq!(a.remote_id, "98329");
        assert_eq!(a.title, "Youjo Senki"); // 优先 romaji
        assert_eq!(a.author.as_deref(), Some("Carlo Zen"));
        assert_eq!(a.language.as_deref(), Some("ja"));
        assert_eq!(a.cover_url.as_deref(), Some("https://img.anili.st/98329.jpg"));
        assert_eq!(a.rights_status, "official_purchase");

        // 仅 native 也算有标题；缺作者/封面/简介 → None。
        let b = &entries[1];
        assert_eq!(b.title, "本好きの下剋上");
        assert!(b.author.is_none() && b.cover_url.is_none() && b.description.is_none());
    }

    #[test]
    fn parse_search_handles_empty_and_garbage() {
        assert!(anilist::parse_search(r#"{"data":{"Page":{"media":[]}}}"#).unwrap().is_empty());
        assert!(anilist::parse_search(r#"{"data":null}"#).unwrap().is_empty());
        assert!(anilist::parse_search("not json").is_err());
    }

    #[test]
    fn request_body_is_valid_json_with_term() {
        let body = anilist::search_request_body("tanya");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["variables"]["search"], "tanya");
        assert!(v["query"].as_str().unwrap().contains("format: NOVEL"));
    }

    #[test]
    fn ingest_creates_remote_entries_visible_in_library() {
        let (dir, conn) = open_db();
        let entries = anilist::parse_search(FIXTURE).unwrap();
        ensure_source(&conn, anilist::SOURCE_ID, anilist::SOURCE_NAME, "metadata", Some(anilist::ENDPOINT), 1000).unwrap();
        let ids = ingest(&conn, anilist::SOURCE_ID, &entries, 1000).unwrap();
        assert_eq!(ids.len(), 2);

        // 远程条目出现在书架，availability=remote、无文件。
        let books = library::list_books(&conn).unwrap();
        assert_eq!(books.len(), 2);
        let tanya = books.iter().find(|b| b.title == "Youjo Senki").unwrap();
        assert_eq!(tanya.availability.as_deref(), Some("remote"));
        assert!(tanya.file_path.is_none());
        assert_eq!(tanya.author.as_deref(), Some("Carlo Zen"));
        // 封面 = AniList URL（read path 从 series.cover_path 取）。
        assert_eq!(tanya.cover_path.as_deref(), Some("https://img.anili.st/98329.jpg"));
        assert_eq!(tanya.id, format!("ed:{}:98329", anilist::SOURCE_ID));
        // 外链经读路径子查询回填到 DTO，前端据此跳官方页。
        assert_eq!(tanya.remote_url.as_deref(), Some("https://anilist.co/manga/98329"));

        // 外链落在 source_record。
        let url: String = conn
            .query_row(
                "SELECT remote_url FROM source_record WHERE entity_id = ?1",
                [&tanya.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(url, "https://anilist.co/manga/98329");

        // 再次 ingest 幂等：不新增条目（upsert 刷新）。
        ingest(&conn, anilist::SOURCE_ID, &entries, 2000).unwrap();
        assert_eq!(library::list_books(&conn).unwrap().len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // 表头故意打散列序 + 含多余列（人物ID/テキストファイルURL）→ 证明按表头名取列。
    // 作品 127 两行（著者芥川 + 译者森）→ 应去重；2000 是 著作権あり。
    const AOZORA_CSV: &str = "人物ID,作品ID,姓,名,作品名,作品著作権フラグ,図書カードURL,テキストファイルURL\n\
1234,127,芥川,龍之介,羅生門,なし,https://www.aozora.gr.jp/cards/000879/card127.html,https://www.aozora.gr.jp/cards/000879/files/127_ruby.zip\n\
5678,127,森,鴎外,羅生門,なし,https://www.aozora.gr.jp/cards/000879/card127.html,https://www.aozora.gr.jp/cards/000879/files/127_ruby.zip\n\
9999,1000,夏目,漱石,吾輩は猫である,なし,https://www.aozora.gr.jp/cards/000148/card1000.html,x\n\
8888,2000,現代,作家,版権あり作品,あり,https://www.aozora.gr.jp/cards/999/card2000.html,y\n";

    #[test]
    fn aozora_parse_filters_dedupes_and_maps_rights() {
        let hit = aozora::parse_catalog_csv(AOZORA_CSV, "羅生門", 50).unwrap();
        assert_eq!(hit.len(), 1, "同作品多行应去重为一条");
        let e = &hit[0];
        assert_eq!(e.remote_id, "127");
        assert_eq!(e.title, "羅生門");
        assert_eq!(e.author.as_deref(), Some("芥川龍之介")); // 姓名相连
        assert_eq!(e.rights_status, "public_domain"); // 著作権フラグ=なし
        assert_eq!(e.language.as_deref(), Some("ja"));
        assert_eq!(e.site_url.as_deref(), Some("https://www.aozora.gr.jp/cards/000879/card127.html"));
        assert!(e.cover_url.is_none() && e.description.is_none());
    }

    #[test]
    fn aozora_parse_empty_query_returns_unique_works_and_flags_copyright() {
        let all = aozora::parse_catalog_csv(AOZORA_CSV, "", 50).unwrap();
        assert_eq!(all.len(), 3, "127/1000/2000 三个作品（127 去重）");
        let copyrighted = all.iter().find(|e| e.remote_id == "2000").unwrap();
        assert_eq!(copyrighted.rights_status, "unknown"); // 著作権あり → 非公共版权
    }

    #[test]
    fn aozora_parse_respects_limit_and_substring() {
        assert_eq!(aozora::parse_catalog_csv(AOZORA_CSV, "", 1).unwrap().len(), 1);
        assert_eq!(aozora::parse_catalog_csv(AOZORA_CSV, "猫", 50).unwrap().len(), 1);
        assert!(aozora::parse_catalog_csv(AOZORA_CSV, "不存在", 50).unwrap().is_empty());
    }

    #[test]
    fn aozora_parse_missing_required_column_errors() {
        let bad = "作品名,図書カードURL\n羅生門,https://x\n"; // 缺作品ID
        assert!(aozora::parse_catalog_csv(bad, "", 50).is_err());
    }

    #[test]
    fn aozora_public_domain_entry_lands_on_shelf() {
        let (dir, conn) = open_db();
        let entries = aozora::parse_catalog_csv(AOZORA_CSV, "羅生門", 50).unwrap();
        ensure_source(&conn, aozora::SOURCE_ID, aozora::SOURCE_NAME, "opds", None, 1000).unwrap();
        let ids = ingest(&conn, aozora::SOURCE_ID, &entries, 1000).unwrap();
        assert_eq!(ids.len(), 1);
        let books = library::list_books(&conn).unwrap();
        let r = books.iter().find(|b| b.title == "羅生門").unwrap();
        assert_eq!(r.availability.as_deref(), Some("remote"));
        assert_eq!(r.remote_url.as_deref(), Some("https://www.aozora.gr.jp/cards/000879/card127.html"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
