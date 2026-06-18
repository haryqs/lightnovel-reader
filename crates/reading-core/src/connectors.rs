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
            params![
                series_id,
                e.title,
                e.author,
                e.description,
                e.cover_url,
                now_ms
            ],
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
               availability = CASE
                 WHEN source_record.availability = 'cached' THEN source_record.availability
                 ELSE excluded.availability
               END,
               last_checked_at = excluded.last_checked_at",
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
        let resp: Resp =
            serde_json::from_str(json).map_err(|e| format!("解析 AniList 响应失败: {e}"))?;
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

/// Bangumi 连接器：中文/ACG 书籍元数据目录，使用公开 OpenAPI，只读取 subject 元数据。
///
/// Bangumi 不是正文来源，也不是可下载来源；这里仅把书籍 subject 映射为远程元数据条目，
/// 点击后跳 Bangumi subject 页面，后续正版购买/阅读入口由用户在外部页面自行判断。
pub mod bangumi {
    use super::RemoteEntry;
    use serde::Deserialize;

    pub const SOURCE_ID: &str = "src:bangumi";
    pub const SOURCE_NAME: &str = "Bangumi";
    pub const ENDPOINT: &str = "https://api.bgm.tv/v0/search/subjects";
    const SUBJECT_BASE_URL: &str = "https://bgm.tv/subject";

    /// 构造 Bangumi subject 搜索请求体。type=1 表示书籍，nsfw=false 避免默认引入成人条目。
    /// 调用方应以 `application/json` POST 到 [`ENDPOINT`] 并用 query 参数控制 limit。
    pub fn search_request_body(term: &str) -> String {
        let body = serde_json::json!({
            "keyword": term,
            "sort": "match",
            "filter": {
                "type": [1],
                "nsfw": false
            }
        });
        body.to_string()
    }

    #[derive(Deserialize)]
    struct Resp {
        data: Option<Vec<Subject>>,
    }

    #[derive(Deserialize)]
    struct Subject {
        id: i64,
        #[serde(rename = "type")]
        subject_type: Option<i64>,
        name: Option<String>,
        name_cn: Option<String>,
        short_summary: Option<String>,
        summary: Option<String>,
        images: Option<Images>,
    }

    #[derive(Deserialize)]
    struct Images {
        large: Option<String>,
        medium: Option<String>,
        common: Option<String>,
        grid: Option<String>,
        small: Option<String>,
    }

    fn clean(v: Option<String>) -> Option<String> {
        v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    }

    fn best_image(images: Images) -> Option<String> {
        images
            .large
            .or(images.medium)
            .or(images.common)
            .or(images.grid)
            .or(images.small)
    }

    /// Parse the official Bangumi search response into remote metadata entries.
    ///
    /// Search already asks for book subjects, but parsing still filters non-book rows defensively.
    pub fn parse_search(json: &str) -> Result<Vec<RemoteEntry>, String> {
        let resp: Resp =
            serde_json::from_str(json).map_err(|e| format!("parse Bangumi response failed: {e}"))?;
        let subjects = resp.data.unwrap_or_default();
        let mut out = Vec::new();

        for s in subjects {
            if s.subject_type.is_some_and(|t| t != 1) {
                continue;
            }
            let title = clean(s.name_cn).or_else(|| clean(s.name));
            let Some(title) = title else {
                continue;
            };

            out.push(RemoteEntry {
                remote_id: s.id.to_string(),
                title,
                author: None,
                description: clean(s.short_summary).or_else(|| clean(s.summary)),
                cover_url: s.images.and_then(best_image),
                language: None,
                site_url: Some(format!("{SUBJECT_BASE_URL}/{}", s.id)),
                // Bangumi 是社区/目录型元数据源，不代表正文授权或购买入口，保守标为 unknown。
                rights_status: "unknown".into(),
            });
        }

        Ok(out)
    }
}

/// 小説家になろう连接器：官方 Web 小说元数据 API。
///
/// なろう比青空更贴近轻小说/网文发现，但这里仍然只做元数据 + 官方外链；
/// 不下载、不清洗、不缓存正文。本模块只解析官方 API JSON（纯函数，可测/可 wasm）。
pub mod narou {
    use super::RemoteEntry;
    use serde::Deserialize;

    pub const SOURCE_ID: &str = "src:narou";
    pub const SOURCE_NAME: &str = "小説家になろう";
    pub const ENDPOINT: &str = "https://api.syosetu.com/novelapi/api/";
    const READER_BASE_URL: &str = "https://ncode.syosetu.com";

    #[derive(Deserialize)]
    struct ApiItem {
        allcount: Option<i64>,
        ncode: Option<String>,
        title: Option<String>,
        writer: Option<String>,
        story: Option<String>,
    }

    fn clean(v: Option<String>) -> Option<String> {
        v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    }

    /// Parse the official Narou JSON response into remote metadata entries.
    ///
    /// The API returns a leading summary object (`{"allcount": ...}`), followed
    /// by novel rows. We skip the summary and any row without an `ncode`/title.
    pub fn parse_search(json: &str) -> Result<Vec<RemoteEntry>, String> {
        let items: Vec<ApiItem> =
            serde_json::from_str(json).map_err(|e| format!("parse Narou response failed: {e}"))?;
        let mut out = Vec::new();

        for item in items {
            if item.allcount.is_some() {
                continue;
            }
            let Some(ncode) = clean(item.ncode) else {
                continue;
            };
            let Some(title) = clean(item.title) else {
                continue;
            };
            let remote_id = ncode.to_ascii_uppercase();
            let reader_code = remote_id.to_ascii_lowercase();

            out.push(RemoteEntry {
                remote_id,
                title,
                author: clean(item.writer),
                description: clean(item.story),
                cover_url: None,
                language: Some("ja".into()),
                site_url: Some(format!("{READER_BASE_URL}/{reader_code}/")),
                rights_status: "official_free".into(),
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

    #[derive(Debug, Clone, PartialEq)]
    pub struct CatalogWork {
        pub remote_id: String,
        pub title: String,
        pub author: Option<String>,
        pub card_url: Option<String>,
        pub text_url: Option<String>,
        pub html_url: Option<String>,
        pub rights_status: String,
    }

    struct CatalogColumns {
        id: usize,
        title: usize,
        copyright: Option<usize>,
        card: Option<usize>,
        last: Option<usize>,
        first: Option<usize>,
        text_url: Option<usize>,
        html_url: Option<usize>,
    }

    fn column(headers: &csv::StringRecord, name: &str) -> Option<usize> {
        headers.iter().position(|h| h == name)
    }

    fn column_any(headers: &csv::StringRecord, names: &[&str]) -> Option<usize> {
        names.iter().find_map(|name| column(headers, name))
    }

    fn columns(headers: &csv::StringRecord) -> Result<CatalogColumns, String> {
        Ok(CatalogColumns {
            id: column(headers, "作品ID").ok_or("青空目录缺『作品ID』列")?,
            title: column(headers, "作品名").ok_or("青空目录缺『作品名』列")?,
            copyright: column(headers, "作品著作権フラグ"),
            card: column(headers, "図書カードURL"),
            last: column(headers, "姓"),
            first: column(headers, "名"),
            text_url: column(headers, "テキストファイルURL"),
            html_url: column_any(headers, &["XHTML/HTMLファイルURL", "HTMLファイルURL"]),
        })
    }

    fn field<'a>(rec: &'a csv::StringRecord, i: Option<usize>) -> Option<&'a str> {
        i.and_then(|i| rec.get(i))
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    fn rights_status(flag: Option<&str>) -> String {
        match flag {
            Some("なし") => "public_domain",
            _ => "unknown",
        }
        .to_string()
    }

    fn author(rec: &csv::StringRecord, cols: &CatalogColumns) -> Option<String> {
        match (field(rec, cols.last), field(rec, cols.first)) {
            (Some(l), Some(f)) => Some(format!("{}{}", l, f)),
            (Some(l), None) => Some(l.to_string()),
            (None, Some(f)) => Some(f.to_string()),
            _ => None,
        }
    }

    fn work_from_record(rec: &csv::StringRecord, cols: &CatalogColumns) -> Option<CatalogWork> {
        let title = field(rec, Some(cols.title))?;
        let remote_id = field(rec, Some(cols.id))?;
        Some(CatalogWork {
            remote_id: remote_id.to_string(),
            title: title.to_string(),
            author: author(rec, cols),
            card_url: field(rec, cols.card).map(str::to_string),
            text_url: field(rec, cols.text_url).map(str::to_string),
            html_url: field(rec, cols.html_url).map(str::to_string),
            rights_status: rights_status(field(rec, cols.copyright)),
        })
    }

    /// 解析官方扩展目录 CSV，按「作品名包含 query」过滤，映射为 [`RemoteEntry`]。
    ///
    /// - **按表头名取列**（作品ID/作品名/作品著作権フラグ/図書カードURL/姓/名），抗列序变化；
    ///   缺必需列（作品ID/作品名）即报错（目录格式变了，早失败）。
    /// - 一个作品在扩展目录里可能多行（著者/翻译者各一行）→ 按作品ID去重，保留首行。
    /// - `著作権フラグ=なし` → `public_domain`；否则 `unknown`。青空作品无封面图、目录无简介。
    /// - `limit` 截断（目录上万行）。
    pub fn parse_catalog_csv(
        csv_text: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<RemoteEntry>, String> {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(csv_text.as_bytes());
        let headers = rdr
            .headers()
            .map_err(|e| format!("青空目录表头解析失败: {e}"))?
            .clone();
        let cols = columns(&headers)?;

        let q = query.trim().to_lowercase();
        let mut seen: HashSet<String> = HashSet::new();
        let mut out = Vec::new();

        for rec in rdr.records() {
            let rec = rec.map_err(|e| format!("青空目录行解析失败: {e}"))?;
            let Some(work) = work_from_record(&rec, &cols) else {
                continue;
            };
            if !q.is_empty() && !work.title.to_lowercase().contains(&q) {
                continue;
            }
            if !seen.insert(work.remote_id.clone()) {
                continue; // 同作品多行去重
            }

            out.push(RemoteEntry {
                remote_id: work.remote_id,
                title: work.title,
                author: work.author,
                description: None,
                cover_url: None,
                language: Some("ja".into()),
                site_url: work.card_url,
                rights_status: work.rights_status,
            });
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    /// 按作品 ID 从官方扩展目录取完整字段，供 PR-B 获取公共版权正文时使用。
    pub fn find_catalog_work_by_id(
        csv_text: &str,
        remote_id: &str,
    ) -> Result<Option<CatalogWork>, String> {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(csv_text.as_bytes());
        let headers = rdr
            .headers()
            .map_err(|e| format!("青空目录表头解析失败: {e}"))?
            .clone();
        let cols = columns(&headers)?;
        for rec in rdr.records() {
            let rec = rec.map_err(|e| format!("青空目录行解析失败: {e}"))?;
            let Some(work) = work_from_record(&rec, &cols) else {
                continue;
            };
            if work.remote_id == remote_id {
                return Ok(Some(work));
            }
        }
        Ok(None)
    }
}

/// OPDS 连接器：解析 OPDS 1.x / 2.0 目录订阅源。
///
/// OPDS（Open Publication Distribution System）是电子出版物目录协议。
/// OPDS 1.x 基于 Atom XML，OPDS 2.0 基于 JSON-LD。
///
/// 本模块只做「XML/JSON 解析 + 条目映射」（纯函数，可测/可 wasm）；
/// HTTP 传输是平台胶水，放在各壳（Tauri command 等）。
pub mod opds {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    use serde::{Deserialize, Serialize};

    /// OPDS 1.x link relation URIs
    const REL_ACQUISITION_PREFIX: &str = "http://opds-spec.org/acquisition";
    const REL_IMAGE: &str = "http://opds-spec.org/image";
    const REL_IMAGE_THUMBNAIL: &str = "http://opds-spec.org/image/thumbnail";
    const REL_ALTERNATE: &str = "alternate";
    #[allow(dead_code)]
    const REL_SELF: &str = "self";
    #[allow(dead_code)]
    const REL_NEXT: &str = "next";
    #[allow(dead_code)]
    const REL_START: &str = "start";
    const REL_SUBSECTION: &str = "subsection";

    /// An OPDS link extracted from a feed or entry.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct OpdsLink {
        pub rel: String,
        pub href: String,
        pub mime_type: Option<String>,
        pub title: Option<String>,
    }

    /// A single entry (publication or navigation link) from an OPDS feed.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct OpdsEntry {
        pub id: String,
        pub title: String,
        pub author: Option<String>,
        pub summary: Option<String>,
        pub links: Vec<OpdsLink>,
        /// Cover image URL (best available from links).
        pub cover_url: Option<String>,
        /// Best acquisition link for EPUB (if available).
        pub acquisition_url: Option<String>,
        /// Whether this entry represents a sub-feed (navigation) rather than a publication.
        pub is_navigation: bool,
    }

    /// A parsed OPDS feed.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct OpdsFeed {
        pub title: String,
        pub entries: Vec<OpdsEntry>,
        pub links: Vec<OpdsLink>,
    }

    /// An OPDS source stored in the library's source table (kind="opds").
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct OpdsSource {
        pub id: String,
        pub name: String,
        pub base_url: Option<String>,
        pub enabled: bool,
    }

    /// List all OPDS sources from the source table.
    pub fn list_sources(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<OpdsSource>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, base_url, enabled FROM source WHERE kind = 'opds' ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(OpdsSource {
                id: row.get(0)?,
                name: row.get(1)?,
                base_url: row.get(2)?,
                enabled: row.get::<_, i64>(3)? != 0,
            })
        })?;
        let mut sources = Vec::new();
        for row in rows {
            sources.push(row?);
        }
        Ok(sources)
    }

    /// Remove an OPDS source by id.
    pub fn remove_source(conn: &rusqlite::Connection, id: &str) -> rusqlite::Result<()> {
        conn.execute("DELETE FROM source WHERE id = ?1 AND kind = 'opds'", [id])?;
        Ok(())
    }

    /// Construct an OPDS 1.x search URL by appending `?q=...` to the base URL.
    /// Real implementations should prefer the feed's OpenSearch link (`rel="search"`),
    /// but this fallback works with many standard OPDS catalogs.
    pub fn search_url(base_url: &str, query: &str) -> String {
        if base_url.contains('?') {
            format!("{}&q={}", base_url, urlencoding(query))
        } else {
            format!("{}?q={}", base_url, urlencoding(query))
        }
    }

    /// Simple percent-encoding for query parameters. Avoids pulling in a URL crate just for this.
    fn urlencoding(s: &str) -> String {
        let mut out = String::with_capacity(s.len() * 3);
        for byte in s.as_bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(*byte as char);
                }
                b' ' => out.push('+'),
                _ => out.push_str(&format!("%{:02X}", byte)),
            }
        }
        out
    }

    impl OpdsEntry {
        /// Convert to a RemoteEntry for ingestion via connectors::ingest.
        pub fn to_remote_entry(&self, source_id: &str) -> super::RemoteEntry {
            // Derive a remote_id from the entry's id + source
            let remote_id = format!("{}:{}", source_id, self.id);
        // Map acquisition link type to rights_status
        let rights_status = if self
            .links
            .iter()
            .any(|l| l.rel.starts_with(REL_ACQUISITION_PREFIX))
        {
            // Has acquisition link → check for free vs restricted
            let is_free = self.links.iter().any(|l| {
                l.rel.starts_with(REL_ACQUISITION_PREFIX)
                    && !l.rel.contains("borrow")
                    && !l.rel.contains("buy")
                    && !l.rel.contains("sample")
                    && l.mime_type
                        .as_deref()
                        .is_some_and(|m| m.contains("epub") || m.contains("pdf"))
            });
            if is_free {
                "open_license"
            } else {
                "unknown"
            }
        } else {
            "metadata_only"
        };

            super::RemoteEntry {
                remote_id,
                title: self.title.clone(),
                author: self.author.clone(),
                description: self.summary.clone(),
                cover_url: self.cover_url.clone(),
                language: None,
                site_url: self
                    .links
                    .iter()
                    .find(|l| l.rel == REL_ALTERNATE)
                    .map(|l| l.href.clone()),
                rights_status: rights_status.to_string(),
            }
        }
    }

    /// Parse an OPDS 1.x (Atom XML) feed string into [`OpdsFeed`].
    ///
    /// Handles both Navigation feeds (entries link to sub-feeds) and Acquisition feeds
    /// (entries are publications with download links).
    pub fn parse_opds_1x(xml: &str) -> Result<OpdsFeed, String> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        reader.config_mut().expand_empty_elements = true;

        let mut feed_title = String::new();
        let mut entries: Vec<OpdsEntry> = Vec::new();
        let mut feed_links: Vec<OpdsLink> = Vec::new();

        // State machine: track which element we're inside
        let mut in_entry = false;
        let mut in_author = false;
        let mut depth: usize = 0;

        // Current entry being built
        let mut entry_id = String::new();
        let mut entry_title = String::new();
        let mut entry_author = String::new();
        let mut entry_summary = String::new();
        let mut entry_links: Vec<OpdsLink> = Vec::new();
        let mut entry_is_nav = false;

        // Track the current XML tag we're inside
        let mut current_tag = String::new();

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    depth += 1;
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    current_tag = tag.clone();

                    if tag == "entry" {
                        in_entry = true;
                        // Begin new entry
                        entry_id.clear();
                        entry_title.clear();
                        entry_author.clear();
                        entry_summary.clear();
                        entry_links.clear();
                        entry_is_nav = false;
                    } else if tag == "author" {
                        in_author = true;
                    }

                    // Extract attributes from link elements
                    if tag == "link" {
                        let rel = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| {
                                String::from_utf8_lossy(a.key.as_ref()).as_ref() == "rel"
                            })
                            .and_then(|a| {
                                String::from_utf8(a.value.to_vec())
                                    .ok()
                            });
                        let href = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| {
                                String::from_utf8_lossy(a.key.as_ref()).as_ref() == "href"
                            })
                            .and_then(|a| {
                                String::from_utf8(a.value.to_vec())
                                    .ok()
                            });
                        let mime_type = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| {
                                String::from_utf8_lossy(a.key.as_ref()).as_ref() == "type"
                            })
                            .and_then(|a| {
                                String::from_utf8(a.value.to_vec())
                                    .ok()
                            });
                        let link_title = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| {
                                String::from_utf8_lossy(a.key.as_ref()).as_ref() == "title"
                            })
                            .and_then(|a| {
                                String::from_utf8(a.value.to_vec())
                                    .ok()
                            });

                        if let (Some(rel), Some(href)) = (rel, href) {
                            let link = OpdsLink {
                                rel: rel.clone(),
                                href,
                                mime_type,
                                title: link_title,
                            };
                            if in_entry {
                                // Check if this entry is a navigation link
                                if rel == REL_SUBSECTION
                                    || (rel.starts_with(REL_ACQUISITION_PREFIX)
                                        && link
                                            .mime_type
                                            .as_deref()
                                            .is_some_and(|m| m.contains("atom")))
                                {
                                    entry_is_nav = true;
                                }
                                entry_links.push(link);
                            } else {
                                feed_links.push(link);
                            }
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    depth = depth.saturating_sub(1);

                    if tag == "entry" {
                        in_entry = false;
                        // Finalize entry
                        let id = if entry_id.is_empty() {
                            // Fallback: use first link href as id
                            entry_links
                                .first()
                                .map(|l| l.href.clone())
                                .unwrap_or_default()
                        } else {
                            entry_id.clone()
                        };

                        // Find best cover image
                        let cover_url = entry_links
                            .iter()
                            .find(|l| {
                                l.rel == REL_IMAGE_THUMBNAIL || l.rel == REL_IMAGE
                            })
                            .or_else(|| {
                                entry_links
                                    .iter()
                                    .find(|l| l.rel == REL_IMAGE_THUMBNAIL)
                            })
                            .map(|l| l.href.clone());

                        // Find best acquisition URL (prefer EPUB)
                        let acquisition_url = entry_links
                            .iter()
                            .find(|l| {
                                l.rel == REL_ACQUISITION_PREFIX
                                    && l.mime_type
                                        .as_deref()
                                        .is_some_and(|m| m.contains("epub"))
                            })
                            .or_else(|| {
                                entry_links
                                    .iter()
                                    .find(|l| l.rel.starts_with(REL_ACQUISITION_PREFIX))
                            })
                            .map(|l| l.href.clone());

                        entries.push(OpdsEntry {
                            id,
                            title: std::mem::take(&mut entry_title),
                            author: if entry_author.is_empty() {
                                None
                            } else {
                                Some(std::mem::take(&mut entry_author))
                            },
                            summary: if entry_summary.is_empty() {
                                None
                            } else {
                                Some(std::mem::take(&mut entry_summary))
                            },
                            links: std::mem::take(&mut entry_links),
                            cover_url,
                            acquisition_url,
                            is_navigation: entry_is_nav,
                        });
                    } else if tag == "author" {
                        in_author = false;
                    }
                }
                Ok(Event::Text(ref e)) => {
                    let text = e.unescape().unwrap_or_default();
                    if !in_entry {
                        if current_tag == "title" {
                            feed_title = text.to_string();
                        }
                    } else if in_entry {
                        match current_tag.as_str() {
                            "id" => entry_id = text.to_string(),
                            "title" => entry_title = text.to_string(),
                            "summary" | "content" => entry_summary = text.to_string(),
                            "name" if in_author => entry_author = text.to_string(),
                            _ => {}
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(e) => {
                    return Err(format!("OPDS XML parse error at position {}: {e}", reader.buffer_position()));
                }
            }
            buf.clear();
        }

        Ok(OpdsFeed {
            title: feed_title,
            entries,
            links: feed_links,
        })
    }

    /// Parse an OPDS 2.0 (JSON / RWPM) feed string into [`OpdsFeed`].
    ///
    /// OPDS 2.0 uses JSON-LD with Readium Web Publication Manifest conventions:
    /// - `metadata.title` → feed title
    /// - `navigation[]` → compact collection of link objects → OpdsEntry(is_navigation=true)
    /// - `publications[]` → publication objects with metadata/links/images → OpdsEntry(is_navigation=false)
    /// - `groups[]` → nested catalogs, flattened into entries
    /// - `facets[]` → ignored (used for filtering/sorting, not content)
    ///
    /// Metadata uses schema.org vocabulary: `title`, `author` (string or {name}),
    /// `description`, `identifier`.
    pub fn parse_opds_2x(json: &str) -> Result<OpdsFeed, String> {
        let root: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("OPDS 2.0 JSON parse error: {e}"))?;

        let feed_title = root
            .get("metadata")
            .and_then(|m| m.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled OPDS Feed")
            .to_string();

        let feed_links = parse_opds2_links(root.get("links"));

        let mut entries: Vec<OpdsEntry> = Vec::new();

        // Navigation links (compact collection)
        if let Some(nav) = root.get("navigation").and_then(|v| v.as_array()) {
            for link_val in nav {
                let entry = opds2_link_to_entry(link_val, true);
                entries.push(entry);
            }
        }

        // Publication entries
        if let Some(pubs) = root.get("publications").and_then(|v| v.as_array()) {
            for pub_val in pubs {
                let entry = opds2_publication_to_entry(pub_val);
                entries.push(entry);
            }
        }

        // Groups: flatten nested navigation/publications
        if let Some(groups) = root.get("groups").and_then(|v| v.as_array()) {
            for group_val in groups {
                let group_title = group_val
                    .get("metadata")
                    .and_then(|m| m.get("title"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Group navigation
                if let Some(nav) = group_val.get("navigation").and_then(|v| v.as_array()) {
                    for link_val in nav {
                        let mut entry = opds2_link_to_entry(link_val, true);
                        if !group_title.is_empty() {
                            entry.title = format!("{} › {}", group_title, entry.title);
                        }
                        entries.push(entry);
                    }
                }

                // Group publications
                if let Some(pubs) = group_val.get("publications").and_then(|v| v.as_array()) {
                    for pub_val in pubs {
                        let mut entry = opds2_publication_to_entry(pub_val);
                        if !group_title.is_empty() {
                            entry.title = format!("{} › {}", group_title, entry.title);
                        }
                        entries.push(entry);
                    }
                }

                // Group self links as nav entries
                if let Some(group_links) = group_val.get("links").and_then(|v| v.as_array()) {
                    for link_val in group_links {
                        let href = link_val
                            .get("href")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let lt = link_val
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or(group_title);
                        if !href.is_empty() && lt != group_title {
                            entries.push(OpdsEntry {
                                id: href.to_string(),
                                title: format!("{} › {}", group_title, lt),
                                author: None,
                                summary: None,
                                links: parse_opds2_links(Some(&serde_json::json!([link_val]))),
                                cover_url: None,
                                acquisition_url: None,
                                is_navigation: true,
                            });
                        }
                    }
                }
            }
        }

        Ok(OpdsFeed {
            title: feed_title,
            entries,
            links: feed_links,
        })
    }

    /// Parse an array of OPDS 2.0 Link Objects into Vec<OpdsLink>.
    fn parse_opds2_links(links_val: Option<&serde_json::Value>) -> Vec<OpdsLink> {
        let arr = match links_val.and_then(|v| v.as_array()) {
            Some(a) => a,
            None => return Vec::new(),
        };
        arr.iter()
            .filter_map(|link| {
                let href = link.get("href").and_then(|v| v.as_str())?;
                let rel = link
                    .get("rel")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                // OPDS 2.0 allows rel to be a string or array of strings
                let rel = if rel.is_empty() {
                    link.get("rel")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                } else {
                    rel
                };
                let mime_type = link.get("type").and_then(|v| v.as_str()).map(|s| s.to_string());
                let title = link.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());
                Some(OpdsLink {
                    rel,
                    href: href.to_string(),
                    mime_type,
                    title,
                })
            })
            .collect()
    }

    /// Convert an OPDS 2.0 Link Object (from navigation or groups) into an OpdsEntry.
    fn opds2_link_to_entry(link: &serde_json::Value, is_nav: bool) -> OpdsEntry {
        let href = link
            .get("href")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let title = link
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or(&href)
            .to_string();
        let links = parse_opds2_links(Some(&serde_json::json!([link])));
        OpdsEntry {
            id: href.clone(),
            title,
            author: None,
            summary: None,
            links,
            cover_url: None,
            acquisition_url: None,
            is_navigation: is_nav,
        }
    }

    /// Convert an OPDS 2.0 Publication object into an OpdsEntry.
    fn opds2_publication_to_entry(pub_val: &serde_json::Value) -> OpdsEntry {
        let meta = pub_val.get("metadata");
        let title = meta
            .and_then(|m| m.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled")
            .to_string();

        let author = meta.and_then(|m| m.get("author")).and_then(|v| {
            // author can be a string or object with "name" field
            v.as_str()
                .map(|s| s.to_string())
                .or_else(|| {
                    v.get("name")
                        .and_then(|n| n.as_str())
                        .or_else(|| v.get("en").and_then(|n| n.as_str()))
                        .map(|s| s.to_string())
                })
        });

        let summary = meta
            .and_then(|m| m.get("description"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let entry_id = meta
            .and_then(|m| m.get("identifier"))
            .and_then(|v| v.as_str())
            .unwrap_or(&title)
            .to_string();

        let links = parse_opds2_links(pub_val.get("links"));

        // Cover from images collection
        let cover_url = pub_val
            .get("images")
            .and_then(|v| v.as_array())
            .and_then(|imgs| {
                // Prefer thumbnail, then any cover image
                imgs.iter()
                    .find(|img| {
                        img.get("rel")
                            .and_then(|r| r.as_str())
                            .is_some_and(|r| r == "http://opds-spec.org/image/thumbnail" || r == "thumbnail")
                    })
                    .or_else(|| {
                        imgs.iter().find(|img| {
                            img.get("rel")
                                .and_then(|r| r.as_str())
                                .is_some_and(|r| r == "cover" || r == "http://opds-spec.org/image")
                        })
                    })
                    .or_else(|| imgs.first())
                    .and_then(|img| img.get("href").and_then(|v| v.as_str()))
                    .map(|s| s.to_string())
            });

        // Acquisition link (prefer EPUB)
        let acquisition_url = links
            .iter()
            .find(|l| {
                l.rel.starts_with(REL_ACQUISITION_PREFIX)
                    && l.mime_type
                        .as_deref()
                        .is_some_and(|m| m.contains("epub"))
            })
            .or_else(|| {
                links
                    .iter()
                    .find(|l| l.rel.starts_with(REL_ACQUISITION_PREFIX))
            })
            .map(|l| l.href.clone());

        OpdsEntry {
            id: entry_id,
            title,
            author,
            summary,
            links,
            cover_url,
            acquisition_url,
            is_navigation: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEMP_ID: AtomicUsize = AtomicUsize::new(0);

    fn open_db() -> (std::path::PathBuf, Connection) {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "reading-core-conntest-{}-{}",
            std::process::id(),
            id
        ));
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
        assert_eq!(
            a.cover_url.as_deref(),
            Some("https://img.anili.st/98329.jpg")
        );
        assert_eq!(a.rights_status, "official_purchase");

        // 仅 native 也算有标题；缺作者/封面/简介 → None。
        let b = &entries[1];
        assert_eq!(b.title, "本好きの下剋上");
        assert!(b.author.is_none() && b.cover_url.is_none() && b.description.is_none());
    }

    #[test]
    fn parse_search_handles_empty_and_garbage() {
        assert!(anilist::parse_search(r#"{"data":{"Page":{"media":[]}}}"#)
            .unwrap()
            .is_empty());
        assert!(anilist::parse_search(r#"{"data":null}"#)
            .unwrap()
            .is_empty());
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
        ensure_source(
            &conn,
            anilist::SOURCE_ID,
            anilist::SOURCE_NAME,
            "metadata",
            Some(anilist::ENDPOINT),
            1000,
        )
        .unwrap();
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
        assert_eq!(
            tanya.cover_path.as_deref(),
            Some("https://img.anili.st/98329.jpg")
        );
        assert_eq!(tanya.id, format!("ed:{}:98329", anilist::SOURCE_ID));
        // 外链经读路径子查询回填到 DTO，前端据此跳官方页。
        assert_eq!(
            tanya.remote_url.as_deref(),
            Some("https://anilist.co/manga/98329")
        );

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
    const AOZORA_CSV: &str = "人物ID,作品ID,姓,名,作品名,作品著作権フラグ,図書カードURL,テキストファイルURL,XHTML/HTMLファイルURL\n\
1234,127,芥川,龍之介,羅生門,なし,https://www.aozora.gr.jp/cards/000879/card127.html,https://www.aozora.gr.jp/cards/000879/files/127_ruby.zip,https://www.aozora.gr.jp/cards/000879/files/127_15260.html\n\
5678,127,森,鴎外,羅生門,なし,https://www.aozora.gr.jp/cards/000879/card127.html,https://www.aozora.gr.jp/cards/000879/files/127_ruby.zip,https://www.aozora.gr.jp/cards/000879/files/127_15260.html\n\
9999,1000,夏目,漱石,吾輩は猫である,なし,https://www.aozora.gr.jp/cards/000148/card1000.html,x,https://www.aozora.gr.jp/cards/000148/files/1000_148.html\n\
8888,2000,現代,作家,版権あり作品,あり,https://www.aozora.gr.jp/cards/999/card2000.html,y,https://www.aozora.gr.jp/cards/999/files/2000.html\n";

    const NAROU_FIXTURE: &str = r#"[
      { "allcount": 2 },
      {
        "ncode": "n1234ab",
        "title": "転生したらテストだった件",
        "writer": "テスト作者",
        "story": "  公式APIから返るあらすじ。  "
      },
      {
        "ncode": "N5678CD",
        "title": "空白の旅人",
        "writer": "",
        "story": null
      },
      {
        "ncode": "N0000ZZ",
        "title": "",
        "writer": "skip",
        "story": "missing title"
      }
    ]"#;

    const BANGUMI_FIXTURE: &str = r#"{
      "data": [
        {
          "id": 26449,
          "type": 1,
          "name": "狼と香辛料",
          "name_cn": "狼与香辛料",
          "short_summary": "旅行商人与贤狼的故事。",
          "images": {
            "large": "https://lain.bgm.tv/pic/cover/l/26449.jpg",
            "common": "https://lain.bgm.tv/pic/cover/c/26449.jpg"
          }
        },
        {
          "id": 123,
          "type": 2,
          "name": "动画条目",
          "name_cn": "",
          "short_summary": "skip non-book"
        },
        {
          "id": 26500,
          "type": 1,
          "name": "Book Without CN",
          "name_cn": "",
          "summary": "fallback summary",
          "images": { "small": "https://lain.bgm.tv/pic/cover/s/26500.jpg" }
        }
      ],
      "total": 3,
      "limit": 20,
      "offset": 0
    }"#;

    #[test]
    fn narou_parse_search_maps_official_free_metadata() {
        let entries = narou::parse_search(NAROU_FIXTURE).unwrap();
        assert_eq!(entries.len(), 2);

        let first = &entries[0];
        assert_eq!(first.remote_id, "N1234AB");
        assert_eq!(first.title, "転生したらテストだった件");
        assert_eq!(first.author.as_deref(), Some("テスト作者"));
        assert_eq!(
            first.description.as_deref(),
            Some("公式APIから返るあらすじ。")
        );
        assert_eq!(first.language.as_deref(), Some("ja"));
        assert_eq!(
            first.site_url.as_deref(),
            Some("https://ncode.syosetu.com/n1234ab/")
        );
        assert_eq!(first.rights_status, "official_free");
        assert!(first.cover_url.is_none());

        let second = &entries[1];
        assert_eq!(second.remote_id, "N5678CD");
        assert!(second.author.is_none());
        assert!(second.description.is_none());
    }

    #[test]
    fn narou_parse_search_handles_empty_and_garbage() {
        assert!(narou::parse_search(r#"[{"allcount":0}]"#)
            .unwrap()
            .is_empty());
        assert!(narou::parse_search("not json").is_err());
    }

    #[test]
    fn bangumi_request_body_is_book_metadata_search() {
        let body = bangumi::search_request_body("狼与香辛料");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["keyword"], "狼与香辛料");
        assert_eq!(v["sort"], "match");
        assert_eq!(v["filter"]["type"][0], 1);
        assert_eq!(v["filter"]["nsfw"], false);
    }

    #[test]
    fn bangumi_parse_search_maps_metadata_and_filters_non_books() {
        let entries = bangumi::parse_search(BANGUMI_FIXTURE).unwrap();
        assert_eq!(entries.len(), 2);

        let first = &entries[0];
        assert_eq!(first.remote_id, "26449");
        assert_eq!(first.title, "狼与香辛料");
        assert_eq!(first.description.as_deref(), Some("旅行商人与贤狼的故事。"));
        assert_eq!(
            first.cover_url.as_deref(),
            Some("https://lain.bgm.tv/pic/cover/l/26449.jpg")
        );
        assert_eq!(
            first.site_url.as_deref(),
            Some("https://bgm.tv/subject/26449")
        );
        assert_eq!(first.rights_status, "unknown");
        assert!(first.author.is_none());
        assert!(first.language.is_none());

        let second = &entries[1];
        assert_eq!(second.remote_id, "26500");
        assert_eq!(second.title, "Book Without CN");
        assert_eq!(second.description.as_deref(), Some("fallback summary"));
        assert_eq!(
            second.cover_url.as_deref(),
            Some("https://lain.bgm.tv/pic/cover/s/26500.jpg")
        );
    }

    #[test]
    fn bangumi_parse_search_handles_empty_and_garbage() {
        assert!(bangumi::parse_search(r#"{"data":[]}"#).unwrap().is_empty());
        assert!(bangumi::parse_search(r#"{"data":null}"#).unwrap().is_empty());
        assert!(bangumi::parse_search("not json").is_err());
    }

    #[test]
    fn bangumi_entry_lands_on_shelf_as_remote_unknown_metadata() {
        let (dir, conn) = open_db();
        let entries = bangumi::parse_search(BANGUMI_FIXTURE).unwrap();
        ensure_source(
            &conn,
            bangumi::SOURCE_ID,
            bangumi::SOURCE_NAME,
            "metadata",
            Some(bangumi::ENDPOINT),
            1000,
        )
        .unwrap();
        let ids = ingest(&conn, bangumi::SOURCE_ID, &entries, 1000).unwrap();
        assert_eq!(ids.len(), 2);

        let books = library::list_books(&conn).unwrap();
        let first = books.iter().find(|b| b.title == "狼与香辛料").unwrap();
        assert_eq!(first.availability.as_deref(), Some("remote"));
        assert_eq!(first.rights_status.as_deref(), Some("unknown"));
        assert_eq!(
            first.remote_url.as_deref(),
            Some("https://bgm.tv/subject/26449")
        );
        assert!(first.file_path.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn narou_entry_lands_on_shelf_as_remote_official_free() {
        let (dir, conn) = open_db();
        let entries = narou::parse_search(NAROU_FIXTURE).unwrap();
        ensure_source(
            &conn,
            narou::SOURCE_ID,
            narou::SOURCE_NAME,
            "metadata",
            Some(narou::ENDPOINT),
            1000,
        )
        .unwrap();
        let ids = ingest(&conn, narou::SOURCE_ID, &entries, 1000).unwrap();
        assert_eq!(ids.len(), 2);

        let books = library::list_books(&conn).unwrap();
        let first = books
            .iter()
            .find(|b| b.title == "転生したらテストだった件")
            .unwrap();
        assert_eq!(first.availability.as_deref(), Some("remote"));
        assert_eq!(first.rights_status.as_deref(), Some("official_free"));
        assert_eq!(
            first.remote_url.as_deref(),
            Some("https://ncode.syosetu.com/n1234ab/")
        );
        assert!(first.file_path.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

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
        assert_eq!(
            e.site_url.as_deref(),
            Some("https://www.aozora.gr.jp/cards/000879/card127.html")
        );
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
        assert_eq!(
            aozora::parse_catalog_csv(AOZORA_CSV, "", 1).unwrap().len(),
            1
        );
        assert_eq!(
            aozora::parse_catalog_csv(AOZORA_CSV, "猫", 50)
                .unwrap()
                .len(),
            1
        );
        assert!(aozora::parse_catalog_csv(AOZORA_CSV, "不存在", 50)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn aozora_parse_missing_required_column_errors() {
        let bad = "作品名,図書カードURL\n羅生門,https://x\n"; // 缺作品ID
        assert!(aozora::parse_catalog_csv(bad, "", 50).is_err());
    }

    #[test]
    fn aozora_find_catalog_work_by_id_returns_body_urls() {
        let work = aozora::find_catalog_work_by_id(AOZORA_CSV, "127")
            .unwrap()
            .expect("应命中作品");
        assert_eq!(work.title, "羅生門");
        assert_eq!(work.rights_status, "public_domain");
        assert_eq!(
            work.text_url.as_deref(),
            Some("https://www.aozora.gr.jp/cards/000879/files/127_ruby.zip")
        );
        assert_eq!(
            work.html_url.as_deref(),
            Some("https://www.aozora.gr.jp/cards/000879/files/127_15260.html")
        );
        assert!(aozora::find_catalog_work_by_id(AOZORA_CSV, "missing")
            .unwrap()
            .is_none());
    }

    #[test]
    fn aozora_public_domain_entry_lands_on_shelf() {
        let (dir, conn) = open_db();
        let entries = aozora::parse_catalog_csv(AOZORA_CSV, "羅生門", 50).unwrap();
        ensure_source(
            &conn,
            aozora::SOURCE_ID,
            aozora::SOURCE_NAME,
            "catalog",
            None,
            1000,
        )
        .unwrap();
        let ids = ingest(&conn, aozora::SOURCE_ID, &entries, 1000).unwrap();
        assert_eq!(ids.len(), 1);
        let books = library::list_books(&conn).unwrap();
        let r = books.iter().find(|b| b.title == "羅生門").unwrap();
        assert_eq!(r.availability.as_deref(), Some("remote"));
        assert_eq!(
            r.remote_url.as_deref(),
            Some("https://www.aozora.gr.jp/cards/000879/card127.html")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── OPDS 1.x parser tests ──

    const OPDS_ACQUISITION_FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"
      xmlns:opds="http://opds-spec.org/2010/catalog"
      xmlns:dcterms="http://purl.org/dc/terms/">
  <id>urn:uuid:feed-1</id>
  <title>Public Domain Books</title>
  <updated>2024-01-01T00:00:00Z</updated>
  <link rel="self" href="https://example.com/opds/root.xml" type="application/atom+xml;profile=opds-catalog"/>
  <entry>
    <id>urn:uuid:book-1</id>
    <title>Pride and Prejudice</title>
    <author>
      <name>Jane Austen</name>
    </author>
    <summary>Classic novel about manners and marriage.</summary>
    <dcterms:language>en</dcterms:language>
    <link rel="http://opds-spec.org/image/thumbnail" href="https://example.com/covers/pp.jpg" type="image/jpeg"/>
    <link rel="http://opds-spec.org/image" href="https://example.com/covers/pp-large.jpg" type="image/jpeg"/>
    <link rel="http://opds-spec.org/acquisition" href="https://example.com/download/pp.epub" type="application/epub+zip"/>
    <link rel="alternate" href="https://example.com/books/pp.html" type="text/html"/>
    <link rel="http://opds-spec.org/acquisition/buy" href="https://store.example.com/pp" type="text/html"/>
  </entry>
  <entry>
    <id>urn:uuid:book-2</id>
    <title>Moby Dick</title>
    <author>
      <name>Herman Melville</name>
    </author>
    <content type="text">The whale story.</content>
    <link rel="http://opds-spec.org/acquisition" href="https://example.com/download/md.epub" type="application/epub+zip"/>
    <link rel="alternate" href="https://example.com/books/md.html" type="text/html"/>
  </entry>
  <entry>
    <id>urn:uuid:book-3</id>
    <title>Metadata Only</title>
    <summary>No acquisition link here.</summary>
    <link rel="http://opds-spec.org/image" href="https://example.com/covers/mo.jpg" type="image/jpeg"/>
  </entry>
</feed>"#;

    const OPDS_NAV_FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"
      xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>urn:uuid:nav-1</id>
  <title>Main Catalog</title>
  <updated>2024-01-01T00:00:00Z</updated>
  <link rel="self" href="https://example.com/opds/" type="application/atom+xml;profile=opds-catalog"/>
  <link rel="start" href="https://example.com/opds/" type="application/atom+xml;profile=opds-catalog"/>
  <entry>
    <title>Classic Literature</title>
    <id>urn:uuid:cat-1</id>
    <content type="text">Public domain classics.</content>
    <link rel="subsection" href="https://example.com/opds/classics.xml" type="application/atom+xml;profile=opds-catalog"/>
  </entry>
  <entry>
    <title>Science Fiction</title>
    <id>urn:uuid:cat-2</id>
    <link rel="subsection" href="https://example.com/opds/scifi.xml" type="application/atom+xml;profile=opds-catalog"/>
  </entry>
</feed>"#;

    #[test]
    fn opds_parse_acquisition_feed_entries_and_links() {
        let feed = opds::parse_opds_1x(OPDS_ACQUISITION_FEED).unwrap();
        assert_eq!(feed.title, "Public Domain Books");
        assert_eq!(feed.entries.len(), 3);

        // Entry 1: full metadata
        let e1 = &feed.entries[0];
        assert_eq!(e1.title, "Pride and Prejudice");
        assert_eq!(e1.author.as_deref(), Some("Jane Austen"));
        assert_eq!(
            e1.summary.as_deref(),
            Some("Classic novel about manners and marriage.")
        );
        assert_eq!(e1.id, "urn:uuid:book-1");
        assert!(!e1.is_navigation);
        assert_eq!(
            e1.cover_url.as_deref(),
            Some("https://example.com/covers/pp.jpg")
        );
        assert_eq!(
            e1.acquisition_url.as_deref(),
            Some("https://example.com/download/pp.epub")
        );
        assert_eq!(e1.links.len(), 5);

        // Entry 2: minimal metadata, no cover
        let e2 = &feed.entries[1];
        assert_eq!(e2.title, "Moby Dick");
        assert_eq!(e2.author.as_deref(), Some("Herman Melville"));
        assert_eq!(e2.summary.as_deref(), Some("The whale story."));
        assert!(e2.cover_url.is_none());
        assert_eq!(
            e2.acquisition_url.as_deref(),
            Some("https://example.com/download/md.epub")
        );

        // Entry 3: no acquisition link
        let e3 = &feed.entries[2];
        assert_eq!(e3.title, "Metadata Only");
        assert!(e3.acquisition_url.is_none());
        assert!(e3.cover_url.is_some());
    }

    #[test]
    fn opds_parse_navigation_feed_detects_subsections() {
        let feed = opds::parse_opds_1x(OPDS_NAV_FEED).unwrap();
        assert_eq!(feed.title, "Main Catalog");
        assert_eq!(feed.entries.len(), 2);

        let e1 = &feed.entries[0];
        assert_eq!(e1.title, "Classic Literature");
        assert!(e1.is_navigation);
        assert_eq!(e1.links.len(), 1);
        assert_eq!(
            e1.links[0].href,
            "https://example.com/opds/classics.xml"
        );

        let e2 = &feed.entries[1];
        assert_eq!(e2.title, "Science Fiction");
        assert!(e2.is_navigation);
    }

    #[test]
    fn opds_to_remote_entry_maps_rights_status() {
        let feed = opds::parse_opds_1x(OPDS_ACQUISITION_FEED).unwrap();
        let e1 = &feed.entries[0];

        // Entry with acquisition → open_license if includes EPUB
        let re1 = e1.to_remote_entry("src:test-opds");
        assert_eq!(re1.rights_status, "open_license");
        assert_eq!(re1.title, "Pride and Prejudice");
        assert_eq!(re1.author.as_deref(), Some("Jane Austen"));
        assert_eq!(
            re1.cover_url.as_deref(),
            Some("https://example.com/covers/pp.jpg")
        );
        assert_eq!(
            re1.site_url.as_deref(),
            Some("https://example.com/books/pp.html")
        );

        // Entry without acquisition → metadata_only
        let e3 = &feed.entries[2];
        let re3 = e3.to_remote_entry("src:test-opds");
        assert_eq!(re3.rights_status, "metadata_only");
    }

    #[test]
    fn opds_parse_handles_empty_and_garbage() {
        let empty =
            opds::parse_opds_1x(r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>Empty</title></feed>"#)
                .unwrap();
        assert_eq!(empty.title, "Empty");
        assert!(empty.entries.is_empty());

        assert!(opds::parse_opds_1x("not xml <").is_err());
    }

    #[test]
    fn opds_entry_lands_on_shelf_as_remote() {
        let (dir, conn) = open_db();
        let feed = opds::parse_opds_1x(OPDS_ACQUISITION_FEED).unwrap();
        let source_id = "src:test-opds";

        // Register source
        ensure_source(
            &conn,
            source_id,
            "Test OPDS",
            "opds",
            Some("https://example.com/opds"),
            1000,
        )
        .unwrap();

        // Convert entries to RemoteEntry and ingest
        let entries: Vec<RemoteEntry> = feed
            .entries
            .iter()
            .map(|e| e.to_remote_entry(source_id))
            .collect();
        let ids = ingest(&conn, source_id, &entries, 1000).unwrap();
        assert_eq!(ids.len(), 3);

        let books = library::list_books(&conn).unwrap();
        assert_eq!(books.len(), 3);

        let pp = books.iter().find(|b| b.title == "Pride and Prejudice").unwrap();
        assert_eq!(pp.availability.as_deref(), Some("remote"));
        assert_eq!(pp.rights_status.as_deref(), Some("open_license"));
        assert_eq!(pp.author.as_deref(), Some("Jane Austen"));
        // Remote entry → cover path = URL
        assert_eq!(
            pp.cover_path.as_deref(),
            Some("https://example.com/covers/pp.jpg")
        );
        // Alternate URL as site_url
        assert_eq!(
            pp.remote_url.as_deref(),
            Some("https://example.com/books/pp.html")
        );

        let mo = books
            .iter()
            .find(|b| b.title == "Metadata Only")
            .unwrap();
        assert_eq!(mo.rights_status.as_deref(), Some("metadata_only"));

        // Verify source_record entries
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_record WHERE source_id = ?1",
                [source_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── OPDS 2.0 JSON tests ──

    const OPDS2_NAV_FEED: &str = r#"{
      "metadata": {"title": "Catalog Root"},
      "links": [
        {"rel": "self", "href": "https://example.com/opds/root.json", "type": "application/opds+json"}
      ],
      "navigation": [
        {"rel": "subsection", "href": "https://example.com/opds/scifi.json", "title": "Science Fiction", "type": "application/opds+json"},
        {"rel": "subsection", "href": "https://example.com/opds/fantasy.json", "title": "Fantasy", "type": "application/opds+json"}
      ]
    }"#;

    const OPDS2_PUB_FEED: &str = r#"{
      "metadata": {"title": "Science Fiction"},
      "links": [
        {"rel": "self", "href": "https://example.com/opds/scifi.json", "type": "application/opds+json"}
      ],
      "publications": [
        {
          "metadata": {
            "identifier": "urn:isbn:9780000000001",
            "title": "Dune",
            "author": "Frank Herbert",
            "description": "Set on the desert planet Arrakis."
          },
          "links": [
            {"rel": "self", "href": "https://example.com/pub/dune.json", "type": "application/webpub+json"},
            {"rel": "http://opds-spec.org/acquisition/open-access", "href": "https://example.com/dune.epub", "type": "application/epub+zip"},
            {"rel": "alternate", "href": "https://example.com/books/dune.html", "title": "Book Page"}
          ],
          "images": [
            {"rel": "http://opds-spec.org/image", "href": "https://example.com/covers/dune.jpg", "type": "image/jpeg"}
          ]
        },
        {
          "metadata": {
            "identifier": "urn:isbn:9780000000002",
            "title": "Foundation",
            "author": {"name": "Isaac Asimov"},
            "description": "A mathematician predicts the fall of the Galactic Empire."
          },
          "links": [
            {"rel": "self", "href": "https://example.com/pub/foundation.json", "type": "application/webpub+json"},
            {"rel": "http://opds-spec.org/acquisition/borrow", "href": "https://example.com/foundation.epub", "type": "application/epub+zip"},
            {"rel": "alternate", "href": "https://example.com/books/foundation.html"}
          ],
          "images": [
            {"rel": "http://opds-spec.org/image/thumbnail", "href": "https://example.com/covers/foundation_thumb.jpg", "type": "image/jpeg"},
            {"rel": "http://opds-spec.org/image", "href": "https://example.com/covers/foundation.jpg", "type": "image/jpeg"}
          ]
        },
        {
          "metadata": {
            "title": "Metadata Only Book"
          },
          "links": [
            {"rel": "self", "href": "https://example.com/pub/mo.json", "type": "application/webpub+json"}
          ]
        }
      ]
    }"#;

    const OPDS2_GROUPS_FEED: &str = r#"{
      "metadata": {"title": "Library Catalog"},
      "groups": [
        {
          "metadata": {"title": "Fiction"},
          "publications": [
            {
              "metadata": {"title": "1984", "author": "George Orwell"},
              "links": [
                {"rel": "self", "href": "https://example.com/pub/1984.json", "type": "application/webpub+json"},
                {"rel": "http://opds-spec.org/acquisition", "href": "https://example.com/1984.epub", "type": "application/epub+zip"}
              ]
            }
          ]
        },
        {
          "metadata": {"title": "Non-Fiction"},
          "publications": [
            {
              "metadata": {"title": "Sapiens", "author": "Yuval Noah Harari"},
              "links": [
                {"rel": "self", "href": "https://example.com/pub/sapiens.json", "type": "application/webpub+json"}
              ]
            }
          ],
          "navigation": [
            {"rel": "subsection", "href": "https://example.com/opds/history.json", "title": "History", "type": "application/opds+json"}
          ]
        }
      ]
    }"#;

    #[test]
    fn opds2_parse_navigation_feed_maps_entries() {
        let feed = opds::parse_opds_2x(OPDS2_NAV_FEED).unwrap();
        assert_eq!(feed.title, "Catalog Root");
        assert_eq!(feed.links.len(), 1);
        assert_eq!(feed.links[0].rel, "self");
        assert_eq!(feed.entries.len(), 2);

        let e0 = &feed.entries[0];
        assert!(e0.is_navigation);
        assert_eq!(e0.title, "Science Fiction");
        assert_eq!(e0.id, "https://example.com/opds/scifi.json");

        let e1 = &feed.entries[1];
        assert!(e1.is_navigation);
        assert_eq!(e1.title, "Fantasy");
    }

    #[test]
    fn opds2_parse_publication_feed_extracts_metadata_and_links() {
        let feed = opds::parse_opds_2x(OPDS2_PUB_FEED).unwrap();
        assert_eq!(feed.title, "Science Fiction");
        assert_eq!(feed.entries.len(), 3);

        // Dune: full metadata, open-access EPUB, image
        let dune = &feed.entries[0];
        assert!(!dune.is_navigation);
        assert_eq!(dune.title, "Dune");
        assert_eq!(dune.author.as_deref(), Some("Frank Herbert"));
        assert!(dune.summary.as_deref().unwrap().contains("Arrakis"));
        assert_eq!(
            dune.acquisition_url.as_deref(),
            Some("https://example.com/dune.epub")
        );
        assert_eq!(
            dune.cover_url.as_deref(),
            Some("https://example.com/covers/dune.jpg")
        );
        // rights: open-access EPUB → open_license
        let re = dune.to_remote_entry("test-opds2");
        assert_eq!(re.rights_status, "open_license");

        // Foundation: author as object {name}, thumbnail preferred for cover
        let found = &feed.entries[1];
        assert_eq!(found.title, "Foundation");
        assert_eq!(found.author.as_deref(), Some("Isaac Asimov"));
        // Thumbnail should be preferred
        assert_eq!(
            found.cover_url.as_deref(),
            Some("https://example.com/covers/foundation_thumb.jpg")
        );
        // borrow EPUB → acquisition present, rights mapped to unknown
        let re = found.to_remote_entry("test-opds2");
        assert_eq!(re.rights_status, "unknown");

        // Metadata-only entry
        let mo = &feed.entries[2];
        assert_eq!(mo.title, "Metadata Only Book");
        assert_eq!(mo.author, None);
        assert_eq!(mo.acquisition_url, None);
        let re = mo.to_remote_entry("test-opds2");
        assert_eq!(re.rights_status, "metadata_only");
    }

    #[test]
    fn opds2_parse_groups_flattens_nested_publications() {
        let feed = opds::parse_opds_2x(OPDS2_GROUPS_FEED).unwrap();
        assert_eq!(feed.title, "Library Catalog");

        // Publication entries should have group-prefixed titles
        let fiction = feed.entries.iter().find(|e| !e.is_navigation && e.title.contains("Fiction")).unwrap();
        assert_eq!(fiction.title, "Fiction › 1984");
        assert_eq!(fiction.author.as_deref(), Some("George Orwell"));

        let nonfiction = feed.entries.iter().find(|e| !e.is_navigation && e.title.contains("Non-Fiction")).unwrap();
        assert_eq!(nonfiction.title, "Non-Fiction › Sapiens");

        // Navigation entry from group
        let hist = feed.entries.iter().find(|e| e.is_navigation && e.title.contains("History")).unwrap();
        assert!(hist.title.contains("Non-Fiction"));
        assert!(hist.title.contains("History"));
    }

    #[test]
    fn opds2_parse_handles_empty_and_garbage() {
        // Empty feed (no navigation or publications)
        let empty = opds::parse_opds_2x(r#"{"metadata":{"title":"Empty"}}"#).unwrap();
        assert_eq!(empty.entries.len(), 0);
        assert_eq!(empty.title, "Empty");

        // Invalid JSON
        assert!(opds::parse_opds_2x("not json").is_err());
        // Empty string
        assert!(opds::parse_opds_2x("").is_err());
    }

    #[test]
    fn opds2_entry_lands_on_shelf_as_remote() {
        use crate::connectors;
        let (dir, conn) = open_db();
        let source_id = "opds2:test-land-on-shelf";

        let feed = opds::parse_opds_2x(OPDS2_PUB_FEED).unwrap();
        connectors::ensure_source(
            &conn,
            source_id,
            "OPDS 2.0 Test Source",
            "opds",
            Some("https://example.com/opds/scifi.json"),
            1000,
        )
        .unwrap();

        // Ingest all entries
        let entries: Vec<RemoteEntry> = feed
            .entries
            .iter()
            .map(|e| e.to_remote_entry(source_id))
            .collect();
        let ids = connectors::ingest(&conn, source_id, &entries, 1000).unwrap();
        assert_eq!(ids.len(), 3);

        let books = library::list_books(&conn).unwrap();
        assert_eq!(books.len(), 3);

        let dune = books.iter().find(|b| b.title == "Dune").unwrap();
        assert_eq!(dune.availability.as_deref(), Some("remote"));
        assert_eq!(dune.rights_status.as_deref(), Some("open_license"));
        assert_eq!(dune.author.as_deref(), Some("Frank Herbert"));
        assert_eq!(
            dune.cover_path.as_deref(),
            Some("https://example.com/covers/dune.jpg")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
