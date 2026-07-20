//! QuickJS 插件运行时：每次调用新建 Runtime/Context，跑完即弃。
//!
//! 插件入口遵循 plugin-sdk：`export default { search, getBook, getChapter }`。
//! HTTP 与 KV 每次调用都重新经过 plugin_host 的权限/域名/尺寸策略门。

#[cfg(feature = "quickjs")]
mod imp {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use rquickjs::{
        promise::MaybePromise, CatchResultExt, Context, Error as JsError, Function, Module, Object,
        Runtime, Value,
    };
    use serde::{Deserialize, Serialize};

    use crate::plugin_host::{
        authorize_acquire_proposal, ensure_kv_access, plan_http_get, AcquireMode, AcquireProposal,
        HostHttpGetPlan, HostHttpGetRequest, HostHttpResponse, PluginBookDetail,
        PluginChapterContent, PluginSearchPage, PluginSearchRequest, MAX_PLUGIN_HTML_INPUT_BYTES,
        MAX_PLUGIN_HTML_SELECTOR_LEN, MAX_PLUGIN_LOG_MESSAGE_BYTES, MAX_PLUGIN_RESULT_JSON_BYTES,
    };
    use crate::plugin_manifest::{is_url_allowed_by_manifest, PluginManifest};

    const RUNTIME_TIMEOUT: Duration = Duration::from_secs(25);
    const RUNTIME_MEMORY_LIMIT: usize = 64 * 1024 * 1024;
    const RUNTIME_STACK_LIMIT: usize = 1024 * 1024;
    const MAX_SOURCE_URL_LEN: usize = 4096;
    const MAX_SOURCE_TITLE_LEN: usize = 512;
    const MAX_SOURCE_TEXT_LEN: usize = 32 * 1024;
    const MAX_SEARCH_RESULTS: usize = 200;
    const MAX_BOOK_CHAPTERS: usize = 20_000;

    fn js_error(message: impl Into<String>) -> JsError {
        JsError::new_from_js_message("plugin host", "JavaScript", message.into())
    }

    /// HTTP 由平台壳执行，core 只负责策略和响应解码。
    pub trait PluginHttpExecutor: Send + Sync {
        fn execute(&self, plan: HostHttpGetPlan) -> Result<HostHttpResponse, String>;
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct PluginTestFlowResult {
        pub search: PluginSearchPage,
        pub book: PluginBookDetail,
        pub chapter: PluginChapterContent,
    }

    pub struct PluginRuntime {
        manifest: PluginManifest,
        entry_js: String,
        http: Arc<dyn PluginHttpExecutor>,
        plugin_root: PathBuf,
        plugin_id: String,
    }

    impl PluginRuntime {
        pub fn new(
            manifest: PluginManifest,
            entry_js: String,
            http: Arc<dyn PluginHttpExecutor>,
            plugin_root: PathBuf,
            plugin_id: String,
        ) -> Self {
            Self {
                manifest,
                entry_js,
                http,
                plugin_root,
                plugin_id,
            }
        }

        /// 调用插件方法。参数是桥接层 JSON，返回值会按 SDK DTO 校验并规范化。
        pub fn call(&self, method: &str, args_json: &str) -> Result<String, String> {
            if !matches!(method, "search" | "getBook" | "getChapter" | "acquire") {
                return Err(format!("当前运行时尚未开放插件方法: {method}"));
            }

            let rt = Runtime::new().map_err(|e| format!("QuickJS Runtime 创建失败: {e}"))?;
            rt.set_memory_limit(RUNTIME_MEMORY_LIMIT);
            rt.set_max_stack_size(RUNTIME_STACK_LIMIT);
            let deadline = Instant::now() + RUNTIME_TIMEOUT;
            rt.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));

            let ctx = Context::full(&rt).map_err(|e| format!("QuickJS Context 创建失败: {e}"))?;
            let source = normalize_entry_module(&self.entry_js);
            let manifest = self.manifest.clone();
            let http = Arc::clone(&self.http);
            let plugin_root = self.plugin_root.clone();
            let plugin_id = self.plugin_id.clone();

            let raw_json = ctx.with(|ctx| -> Result<String, String> {
                inject_host_api(&ctx, &manifest, http, &plugin_root, &plugin_id, deadline)?;
                inject_polyfills(&ctx)?;

                let (module, evaluated) = Module::declare(ctx.clone(), "plugin-entry.js", source)
                    .map_err(|e| format!("插件入口编译失败: {e}"))?
                    .eval()
                    .catch(&ctx)
                    .map_err(|e| format!("插件入口执行失败: {e}"))?;
                evaluated
                    .finish::<()>()
                    .catch(&ctx)
                    .map_err(|e| format!("插件入口异步初始化失败: {e}"))?;

                let plugin: Object = module
                    .get("default")
                    .map_err(|_| "插件入口必须 export default 一个对象".to_string())?;
                let function: Function = plugin
                    .get(method)
                    .map_err(|_| format!("插件未导出方法: {method}"))?;

                let pending = invoke_sdk_method(&function, method, args_json)?;
                let value: Value = pending
                    .finish()
                    .catch(&ctx)
                    .map_err(|e| format!("插件调用失败: {e}"))?;
                let encoded = ctx
                    .json_stringify(value)
                    .catch(&ctx)
                    .map_err(|e| format!("结果序列化失败: {e}"))?
                    .ok_or_else(|| "插件返回值无法序列化为 JSON".to_string())?;
                encoded
                    .to_string()
                    .map_err(|e| format!("结果字符串读取失败: {e}"))
            })?;

            if raw_json.len() > MAX_PLUGIN_RESULT_JSON_BYTES {
                return Err(format!(
                    "插件返回 JSON 超过 {} 字节上限",
                    MAX_PLUGIN_RESULT_JSON_BYTES
                ));
            }

            validate_method_result(&self.manifest, method, &raw_json)
        }

        pub fn search(&self, query: &str, page: u32) -> Result<PluginSearchPage, String> {
            let query = query.trim();
            if query.is_empty() {
                return Err("插件搜索关键词不能为空".into());
            }
            if page == 0 {
                return Err("插件搜索页码必须从 1 开始".into());
            }
            let json = self.call(
                "search",
                &serde_json::to_string(&PluginSearchRequest {
                    query: query.to_string(),
                    page,
                })
                .map_err(|e| e.to_string())?,
            )?;
            serde_json::from_str(&json).map_err(|e| format!("搜索结果解析失败: {e}"))
        }

        pub fn get_book(&self, book_url: &str) -> Result<PluginBookDetail, String> {
            validate_source_url(&self.manifest, book_url, "bookUrl")?;
            let json = self.call(
                "getBook",
                &serde_json::json!({ "bookUrl": book_url }).to_string(),
            )?;
            serde_json::from_str(&json).map_err(|e| format!("书籍详情解析失败: {e}"))
        }

        pub fn get_chapter(&self, chapter_url: &str) -> Result<PluginChapterContent, String> {
            validate_source_url(&self.manifest, chapter_url, "chapterUrl")?;
            let json = self.call(
                "getChapter",
                &serde_json::json!({ "chapterUrl": chapter_url }).to_string(),
            )?;
            serde_json::from_str(&json).map_err(|e| format!("章节结果解析失败: {e}"))
        }

        pub fn acquire(
            &self,
            remote_id: &str,
            mode: AcquireMode,
        ) -> Result<AcquireProposal, String> {
            let remote_id = remote_id.trim();
            if remote_id.is_empty() || remote_id.len() > MAX_SOURCE_URL_LEN {
                return Err(format!("插件获取标识必须是 1..={MAX_SOURCE_URL_LEN} 字节"));
            }
            let json = self.call(
                "acquire",
                &serde_json::to_string(&AcquireRequest {
                    remote_id: remote_id.to_string(),
                    mode,
                })
                .map_err(|e| e.to_string())?,
            )?;
            let proposal: AcquireProposal =
                serde_json::from_str(&json).map_err(|e| format!("获取提案解析失败: {e}"))?;
            authorize_acquire_proposal(&self.manifest, mode, proposal.clone())?;
            Ok(proposal)
        }

        /// 以第一个搜索结果和第一章跑通 SDK 三个必选方法。
        pub fn run_test_flow(&self, query: &str) -> Result<PluginTestFlowResult, String> {
            let search = self.search(query, 1)?;
            let first_url = search
                .results
                .first()
                .map(|result| result.url.clone())
                .ok_or_else(|| "插件搜索没有返回可用于测试的结果".to_string())?;

            let book = self.get_book(&first_url)?;
            let first_chapter_url = book
                .chapters
                .first()
                .map(|chapter| chapter.url.clone())
                .ok_or_else(|| "插件书籍详情没有返回可用于测试的章节".to_string())?;

            let chapter = self.get_chapter(&first_chapter_url)?;

            Ok(PluginTestFlowResult {
                search,
                book,
                chapter,
            })
        }
    }

    fn normalize_entry_module(entry: &str) -> Vec<u8> {
        if entry.contains("export default") {
            entry.as_bytes().to_vec()
        } else {
            format!("{entry}\nexport default plugin;\n").into_bytes()
        }
    }

    fn invoke_sdk_method<'js>(
        function: &Function<'js>,
        method: &str,
        args_json: &str,
    ) -> Result<MaybePromise<'js>, String> {
        match method {
            "search" => {
                let request: PluginSearchRequest =
                    serde_json::from_str(args_json).map_err(|e| format!("search 参数无效: {e}"))?;
                function
                    .call((request.query, request.page))
                    .map_err(|e| format!("search 调用失败: {e}"))
            }
            "getBook" => {
                let request: BookRequest = serde_json::from_str(args_json)
                    .map_err(|e| format!("getBook 参数无效: {e}"))?;
                function
                    .call((request.book_url,))
                    .map_err(|e| format!("getBook 调用失败: {e}"))
            }
            "getChapter" => {
                let request: ChapterRequest = serde_json::from_str(args_json)
                    .map_err(|e| format!("getChapter 参数无效: {e}"))?;
                function
                    .call((request.chapter_url,))
                    .map_err(|e| format!("getChapter 调用失败: {e}"))
            }
            "acquire" => {
                let request: AcquireRequest = serde_json::from_str(args_json)
                    .map_err(|e| format!("acquire 参数无效: {e}"))?;
                let mode = match request.mode {
                    AcquireMode::MetadataOnly => "metadataOnly",
                    AcquireMode::Download => "download",
                    AcquireMode::CacheForReading => "cacheForReading",
                };
                function
                    .call((request.remote_id, mode))
                    .map_err(|e| format!("acquire 调用失败: {e}"))
            }
            _ => Err(format!("不支持的插件方法: {method}")),
        }
    }

    fn validate_method_result(
        manifest: &PluginManifest,
        method: &str,
        json: &str,
    ) -> Result<String, String> {
        match method {
            "search" => {
                let page: PluginSearchPage = parse_method_json(json, "search")?;
                validate_search_page(manifest, &page)?;
                serde_json::to_string(&page).map_err(|e| e.to_string())
            }
            "getBook" => {
                let book: PluginBookDetail = parse_method_json(json, "getBook")?;
                validate_book_detail(manifest, &book)?;
                serde_json::to_string(&book).map_err(|e| e.to_string())
            }
            "getChapter" => {
                let mut chapter: PluginChapterContent = serde_json::from_str(json)
                    .map_err(|e| format!("getChapter 返回值不符合 SDK: {e}"))?;
                chapter.html = crate::html_sanitizer::sanitize_fragment(&chapter.html);
                validate_text(&chapter.title, "章节标题", MAX_SOURCE_TITLE_LEN, false)?;
                if chapter.html.trim().is_empty() {
                    return Err("getChapter 返回的章节正文不能为空".into());
                }
                serde_json::to_string(&chapter).map_err(|e| e.to_string())
            }
            "acquire" => {
                let proposal: AcquireProposal = parse_method_json(json, "acquire")?;
                validate_source_url(manifest, &proposal.url, "获取 URL")?;
                validate_optional_text(
                    proposal.mime_type.as_deref(),
                    "获取 MIME 类型",
                    MAX_SOURCE_TITLE_LEN,
                )?;
                validate_optional_text(proposal.note.as_deref(), "获取说明", MAX_SOURCE_TEXT_LEN)?;
                serde_json::to_string(&proposal).map_err(|e| e.to_string())
            }
            _ => Err(format!("不支持的插件方法: {method}")),
        }
    }

    fn parse_method_json<T>(json: &str, method: &str) -> Result<T, String>
    where
        T: for<'de> Deserialize<'de>,
    {
        serde_json::from_str(json).map_err(|e| format!("{method} 返回值不符合 SDK: {e}"))
    }

    fn validate_search_page(
        manifest: &PluginManifest,
        page: &PluginSearchPage,
    ) -> Result<(), String> {
        if page.results.len() > MAX_SEARCH_RESULTS {
            return Err(format!("search 返回结果超过 {MAX_SEARCH_RESULTS} 条上限"));
        }
        for (index, result) in page.results.iter().enumerate() {
            validate_source_url(manifest, &result.url, &format!("搜索结果 {index} URL"))?;
            validate_optional_source_url(
                manifest,
                result.cover_url.as_deref(),
                &format!("搜索结果 {index} 封面 URL"),
            )?;
            validate_text(
                &result.title,
                &format!("搜索结果 {index} 标题"),
                MAX_SOURCE_TITLE_LEN,
                false,
            )?;
            validate_optional_text(
                result.author.as_deref(),
                &format!("搜索结果 {index} 作者"),
                MAX_SOURCE_TITLE_LEN,
            )?;
            validate_optional_text(
                result.summary.as_deref(),
                &format!("搜索结果 {index} 简介"),
                MAX_SOURCE_TEXT_LEN,
            )?;
        }
        Ok(())
    }

    fn validate_book_detail(
        manifest: &PluginManifest,
        book: &PluginBookDetail,
    ) -> Result<(), String> {
        validate_source_url(manifest, &book.url, "书籍 URL")?;
        validate_optional_source_url(manifest, book.cover_url.as_deref(), "书籍封面 URL")?;
        validate_text(&book.title, "书籍标题", MAX_SOURCE_TITLE_LEN, false)?;
        validate_optional_text(book.author.as_deref(), "书籍作者", MAX_SOURCE_TITLE_LEN)?;
        validate_optional_text(book.description.as_deref(), "书籍简介", MAX_SOURCE_TEXT_LEN)?;
        if book.chapters.len() > MAX_BOOK_CHAPTERS {
            return Err(format!("getBook 返回章节超过 {MAX_BOOK_CHAPTERS} 条上限"));
        }
        for (index, chapter) in book.chapters.iter().enumerate() {
            validate_source_url(manifest, &chapter.url, &format!("章节 {index} URL"))?;
            validate_text(
                &chapter.title,
                &format!("章节 {index} 标题"),
                MAX_SOURCE_TITLE_LEN,
                false,
            )?;
            validate_optional_text(
                chapter.group.as_deref(),
                &format!("章节 {index} 分组"),
                MAX_SOURCE_TITLE_LEN,
            )?;
        }
        Ok(())
    }

    fn validate_source_url(
        manifest: &PluginManifest,
        url: &str,
        label: &str,
    ) -> Result<(), String> {
        if url.is_empty() || url.len() > MAX_SOURCE_URL_LEN {
            return Err(format!("{label} 必须是 1..={MAX_SOURCE_URL_LEN} 字节"));
        }
        if !is_url_allowed_by_manifest(manifest, url) {
            return Err(format!("{label} 不在 manifest 域名白名单内"));
        }
        Ok(())
    }

    fn validate_optional_source_url(
        manifest: &PluginManifest,
        url: Option<&str>,
        label: &str,
    ) -> Result<(), String> {
        if let Some(url) = url {
            validate_source_url(manifest, url, label)?;
        }
        Ok(())
    }

    fn validate_text(
        value: &str,
        label: &str,
        max_chars: usize,
        allow_empty: bool,
    ) -> Result<(), String> {
        let count = value.chars().count();
        if (!allow_empty && value.trim().is_empty()) || count > max_chars {
            return Err(format!("{label} 必须是 1..={max_chars} 个字符"));
        }
        Ok(())
    }

    fn validate_optional_text(
        value: Option<&str>,
        label: &str,
        max_chars: usize,
    ) -> Result<(), String> {
        if let Some(value) = value {
            validate_text(value, label, max_chars, true)?;
        }
        Ok(())
    }

    fn inject_host_api(
        ctx: &rquickjs::Ctx<'_>,
        manifest: &PluginManifest,
        http: Arc<dyn PluginHttpExecutor>,
        plugin_root: &Path,
        plugin_id: &str,
        deadline: Instant,
    ) -> Result<(), String> {
        let http_manifest = manifest.clone();
        let http_fn = Function::new(
            ctx.clone(),
            move |url: String, options_json: String| -> rquickjs::Result<String> {
                let options: RuntimeHttpOptions = if options_json.trim().is_empty() {
                    RuntimeHttpOptions::default()
                } else {
                    serde_json::from_str(&options_json)
                        .map_err(|e| js_error(format!("host.http options 无效: {e}")))?
                };
                let mut plan = plan_http_get(
                    &http_manifest,
                    HostHttpGetRequest {
                        url,
                        headers: options.headers,
                        timeout_ms: options.timeout_ms,
                    },
                )
                .map_err(js_error)?;
                let remaining = deadline
                    .checked_duration_since(Instant::now())
                    .ok_or_else(|| js_error("插件调用已超时"))?;
                plan.timeout_ms = plan
                    .timeout_ms
                    .min(remaining.as_millis().max(1).min(u64::MAX as u128) as u64);
                let response = http.execute(plan).map_err(js_error)?;
                if Instant::now() >= deadline {
                    return Err(js_error("插件 HTTP 调用已超时"));
                }
                let text = decode_http_body(&response).map_err(js_error)?;
                serde_json::to_string(&RuntimeHttpResponse {
                    status: response.status,
                    headers: response.headers,
                    body: text,
                })
                .map_err(|e| js_error(e.to_string()))
            },
        )
        .map_err(|e| e.to_string())?;
        ctx.globals()
            .set("__hostHttpGet", http_fn)
            .map_err(|e| e.to_string())?;

        let kv_manifest = manifest.clone();
        let root = plugin_root.to_path_buf();
        let id = plugin_id.to_string();
        let kv_get = Function::new(
            ctx.clone(),
            move |key: String| -> rquickjs::Result<Option<String>> {
                ensure_kv_access(&kv_manifest, &key, None).map_err(js_error)?;
                Ok(crate::plugin_store::plugin_kv_get(&root, &id, &key))
            },
        )
        .map_err(|e| e.to_string())?;
        ctx.globals()
            .set("__hostKvGet", kv_get)
            .map_err(|e| e.to_string())?;

        let kv_manifest = manifest.clone();
        let root = plugin_root.to_path_buf();
        let id = plugin_id.to_string();
        let kv_set = Function::new(
            ctx.clone(),
            move |key: String, value: String| -> rquickjs::Result<()> {
                ensure_kv_access(&kv_manifest, &key, Some(&value)).map_err(js_error)?;
                crate::plugin_store::plugin_kv_set(&root, &id, &key, &value).map_err(js_error)
            },
        )
        .map_err(|e| e.to_string())?;
        ctx.globals()
            .set("__hostKvSet", kv_set)
            .map_err(|e| e.to_string())?;

        let kv_manifest = manifest.clone();
        let root = plugin_root.to_path_buf();
        let id = plugin_id.to_string();
        let kv_delete = Function::new(ctx.clone(), move |key: String| -> rquickjs::Result<()> {
            ensure_kv_access(&kv_manifest, &key, None).map_err(js_error)?;
            crate::plugin_store::plugin_kv_delete(&root, &id, &key).map_err(js_error)
        })
        .map_err(|e| e.to_string())?;
        ctx.globals()
            .set("__hostKvDelete", kv_delete)
            .map_err(|e| e.to_string())?;

        let html_query = Function::new(
            ctx.clone(),
            |html: String, selector: String| -> rquickjs::Result<String> {
                query_html(&html, &selector).map_err(js_error)
            },
        )
        .map_err(|e| e.to_string())?;
        ctx.globals()
            .set("__hostHtmlQuery", html_query)
            .map_err(|e| e.to_string())?;

        let log = Function::new(
            ctx.clone(),
            |level: String, message: String| -> rquickjs::Result<()> {
                if message.len() > MAX_PLUGIN_LOG_MESSAGE_BYTES {
                    return Err(js_error(format!(
                        "插件日志超过 {} 字节上限",
                        MAX_PLUGIN_LOG_MESSAGE_BYTES
                    )));
                }
                eprintln!("[plugin:{level}] {message}");
                Ok(())
            },
        )
        .map_err(|e| e.to_string())?;
        ctx.globals()
            .set("__hostLog", log)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn inject_polyfills(ctx: &rquickjs::Ctx<'_>) -> Result<(), String> {
        ctx.eval::<(), _>(
            r#"
            (function () {
              async function httpGet(url, opts) {
                var raw = JSON.parse(__hostHttpGet(String(url), JSON.stringify(opts || {})))
                return {
                  status: raw.status,
                  headers: raw.headers,
                  text: function () { return raw.body }
                }
              }

              function wrapElement(data) {
                return {
                  text: data.text,
                  innerHtml: data.innerHtml,
                  attr: function (name) {
                    return Object.prototype.hasOwnProperty.call(data.attributes, name)
                      ? data.attributes[name]
                      : null
                  },
                  select: function (selector) { return htmlQuery(data.innerHtml, selector) },
                  selectFirst: function (selector) {
                    var found = htmlQuery(data.innerHtml, selector)
                    return found.length ? found[0] : null
                  }
                }
              }

              function htmlQuery(html, selector) {
                return JSON.parse(__hostHtmlQuery(String(html), String(selector))).map(wrapElement)
              }

              function makeLog(level) {
                return function () {
                  var args = Array.prototype.slice.call(arguments)
                  __hostLog(level, args.map(function (value) {
                    if (typeof value === 'string') return value
                    try { return JSON.stringify(value) } catch (_) { return String(value) }
                  }).join(' '))
                }
              }

              globalThis.host = {
                http: { get: httpGet },
                html: {
                  parse: function (html) {
                    return {
                      select: function (selector) { return htmlQuery(html, selector) },
                      selectFirst: function (selector) {
                        var found = htmlQuery(html, selector)
                        return found.length ? found[0] : null
                      }
                    }
                  }
                },
                kv: {
                  get: async function (key) { return __hostKvGet(String(key)) },
                  set: async function (key, value) { __hostKvSet(String(key), String(value)) },
                  delete: async function (key) { __hostKvDelete(String(key)) }
                },
                log: { info: makeLog('info'), warn: makeLog('warn'), error: makeLog('error') }
              }

              if (typeof URL === 'undefined') {
                globalThis.URL = function (input, base) {
                  var value = String(input || '')
                  if (!/^https?:\/\//i.test(value) && base) {
                    var root = String(base)
                    if (value.indexOf('/') === 0) {
                      var origin = root.match(/^(https?:\/\/[^/]+)/i)
                      value = origin ? origin[1] + value : value
                    } else {
                      value = root.replace(/[^/]*$/, '') + value
                    }
                  }
                  var match = value.match(/^(https?:)\/\/([^/]+)(\/[^?#]*)?(\?[^#]*)?(#.*)?$/i)
                  if (!match) throw new TypeError('Invalid URL')
                  this.href = value
                  this.protocol = match[1]
                  this.hostname = match[2]
                  this.pathname = match[3] || '/'
                  this.search = match[4] || ''
                  this.hash = match[5] || ''
                  this.toString = function () { return this.href }
                }
              }

              if (typeof TextDecoder === 'undefined') {
                globalThis.TextDecoder = function () {
                  this.decode = function (bytes) {
                    var out = ''
                    for (var i = 0; i < bytes.length; i++) out += String.fromCharCode(bytes[i])
                    return out
                  }
                }
              }

              delete globalThis.eval
            })()
            "#,
        )
        .catch(ctx)
        .map_err(|e| format!("宿主 API 注入失败: {e}"))
    }

    fn query_html(html: &str, selector: &str) -> Result<String, String> {
        if html.len() > MAX_PLUGIN_HTML_INPUT_BYTES {
            return Err(format!(
                "HTML 输入超过 {} 字节上限",
                MAX_PLUGIN_HTML_INPUT_BYTES
            ));
        }
        if selector.is_empty() || selector.len() > MAX_PLUGIN_HTML_SELECTOR_LEN {
            return Err(format!(
                "CSS 选择器必须为 1..={} 字节",
                MAX_PLUGIN_HTML_SELECTOR_LEN
            ));
        }
        let dom = tl::parse(html, tl::ParserOptions::default())
            .map_err(|e| format!("HTML 解析失败: {e}"))?;
        let parser = dom.parser();
        let mut elements = Vec::new();
        let mut seen = BTreeSet::new();

        for part in selector
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            let matches = dom
                .query_selector(part)
                .ok_or_else(|| format!("不支持的 CSS 选择器: {part}"))?;
            for handle in matches {
                let Some(tag) = handle.get(parser).and_then(|node| node.as_tag()) else {
                    continue;
                };
                let outer_html = tag.outer_html(parser);
                if !seen.insert(outer_html) {
                    continue;
                }
                let attributes = tag
                    .attributes()
                    .iter()
                    .map(|(name, value)| {
                        (
                            name.into_owned(),
                            value.map_or_else(String::new, |v| v.into_owned()),
                        )
                    })
                    .collect();
                elements.push(HtmlElementSnapshot {
                    text: normalize_html_text(&tag.inner_text(parser)),
                    inner_html: tag.inner_html(parser),
                    attributes,
                });
            }
        }
        serde_json::to_string(&elements).map_err(|e| e.to_string())
    }

    fn normalize_html_text(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn decode_http_body(response: &HostHttpResponse) -> Result<String, String> {
        let content_type = response
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .map(|(_, value)| value.as_str());
        let declared = content_type.and_then(|value| {
            value.split(';').skip(1).find_map(|part| {
                let (name, label) = part.trim().split_once('=')?;
                name.eq_ignore_ascii_case("charset")
                    .then(|| label.trim_matches(['\'', '"', ' ']))
            })
        });
        if let Some(encoding) =
            declared.and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes()))
        {
            let (decoded, _, _) = encoding.decode(&response.body);
            return Ok(decoded.into_owned());
        }
        crate::epub_parser::decode_text(&response.body)
    }

    #[derive(Debug, Default, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RuntimeHttpOptions {
        #[serde(default)]
        headers: BTreeMap<String, String>,
        #[serde(default)]
        timeout_ms: Option<u64>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct RuntimeHttpResponse {
        status: u16,
        headers: BTreeMap<String, String>,
        body: String,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct HtmlElementSnapshot {
        text: String,
        inner_html: String,
        attributes: BTreeMap<String, String>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BookRequest {
        book_url: String,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ChapterRequest {
        chapter_url: String,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct AcquireRequest {
        remote_id: String,
        mode: AcquireMode,
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::plugin_manifest::{
            PluginCapability, PluginLegal, PluginLegalKind, PluginPermission,
        };
        use std::time::{SystemTime, UNIX_EPOCH};

        struct FixtureHttp;

        impl PluginHttpExecutor for FixtureHttp {
            fn execute(&self, plan: HostHttpGetPlan) -> Result<HostHttpResponse, String> {
                let body = match plan.url.as_str() {
                    "https://example.com/search" => {
                        r#"<main><a class="book" href="https://example.com/book/1">Example Book</a></main>"#
                    }
                    "https://example.com/book/1" => {
                        r#"<article><h1>Example Book</h1><a class="chapter" href="https://example.com/chapter/1">Chapter One</a></article>"#
                    }
                    "https://example.com/chapter/1" => {
                        r#"<article><h1>Chapter One</h1><p>Hello <strong>QuickJS</strong>.</p><script>alert(1)</script></article>"#
                    }
                    other => return Err(format!("unexpected fixture URL: {other}")),
                };
                Ok(HostHttpResponse {
                    status: 200,
                    headers: BTreeMap::from([(
                        "content-type".into(),
                        "text/html; charset=utf-8".into(),
                    )]),
                    body: body.as_bytes().to_vec(),
                })
            }
        }

        fn manifest() -> PluginManifest {
            PluginManifest {
                api_version: "0.1".into(),
                id: "runtime-test".into(),
                name: "Runtime Test".into(),
                version: "0.1.0".into(),
                description: None,
                author: None,
                language: Some("en".into()),
                entry: "plugin.js".into(),
                domains: vec!["example.com".into()],
                permissions: vec![PluginPermission::Http, PluginPermission::Kv],
                capabilities: vec![PluginCapability::Acquire],
                legal: PluginLegal {
                    kind: PluginLegalKind::PublicDomain,
                    note: None,
                    terms_url: None,
                },
            }
        }

        fn entry() -> String {
            r#"
                export default {
                  async search(query, page) {
                    const response = await host.http.get('https://example.com/search').then(value => value)
                    const doc = host.html.parse(response.text())
                    const savedCount = await host.kv.get('count').then(value => value)
                    const count = Number(savedCount || '0') + 1
                    await host.kv.set('count', String(count))
                    return {
                      results: doc.select('.book').map((item) => ({
                        url: item.attr('href'),
                        title: item.text + ' ' + query + ' #' + count
                      })),
                      hasMore: page < 1
                    }
                  },
                  async getBook(bookUrl) {
                    const response = await host.http.get(bookUrl)
                    const doc = host.html.parse(response.text())
                    const chapter = doc.selectFirst('.chapter')
                    return {
                      url: bookUrl,
                      title: doc.selectFirst('h1').text,
                      chapters: [{ url: chapter.attr('href'), title: chapter.text }]
                    }
                  },
                  async getChapter(chapterUrl) {
                    const response = await host.http.get(chapterUrl)
                    const doc = host.html.parse(response.text())
                    return {
                      title: doc.selectFirst('h1').text,
                      html: doc.selectFirst('article').innerHtml
                    }
                  },
                  async acquire(remoteId, mode) {
                    return {
                      url: 'https://example.com/book.epub',
                      rightsStatus: 'public_domain',
                      mimeType: 'application/epub+zip',
                      note: remoteId + ' ' + mode
                    }
                  }
                }
            "#
            .into()
        }

        fn plugin_root() -> PathBuf {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir()
                .join(format!("lnr-plugin-runtime-{}-{nonce}", std::process::id()));
            std::fs::create_dir_all(root.join("runtime-test")).unwrap();
            root
        }

        #[test]
        fn runs_async_sdk_flow_with_http_html_kv_and_sanitizing() {
            let root = plugin_root();
            let runtime = PluginRuntime::new(
                manifest(),
                entry(),
                Arc::new(FixtureHttp),
                root.clone(),
                "runtime-test".into(),
            );

            let result = runtime.run_test_flow("query").unwrap();
            assert_eq!(result.search.results.len(), 1);
            assert!(result.search.results[0].title.contains("query #1"));
            assert_eq!(result.book.title, "Example Book");
            assert_eq!(result.chapter.title, "Chapter One");
            assert!(result.chapter.html.contains("<strong>QuickJS</strong>"));
            assert!(!result.chapter.html.contains("script"));

            let second = runtime.run_test_flow("again").unwrap();
            assert!(second.search.results[0].title.contains("again #2"));
            let proposal = runtime
                .acquire("https://example.com/book/1", AcquireMode::CacheForReading)
                .unwrap();
            assert_eq!(proposal.url, "https://example.com/book.epub");
            assert_eq!(proposal.mime_type.as_deref(), Some("application/epub+zip"));
            assert!(proposal.note.unwrap().contains("cacheForReading"));
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn rejects_result_urls_outside_manifest_domains() {
            let root = plugin_root();
            let runtime = PluginRuntime::new(
                manifest(),
                r#"
                    export default {
                      async search() {
                        return {
                          results: [{ url: 'javascript:alert(1)', title: 'Unsafe' }],
                          hasMore: false
                        }
                      },
                      async getBook(bookUrl) {
                        return { url: bookUrl, title: 'Unsafe', chapters: [] }
                      },
                      async getChapter() {
                        return { title: 'Unsafe', html: '<p>Unsafe</p>' }
                      }
                    }
                "#
                .into(),
                Arc::new(FixtureHttp),
                root.clone(),
                "runtime-test".into(),
            );

            let error = runtime.search("unsafe", 1).unwrap_err();
            assert!(error.contains("manifest 域名白名单"));
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn acquire_rejects_out_of_domain_and_rights_escalation() {
            let root = plugin_root();
            let out_of_domain = PluginRuntime::new(
                manifest(),
                r#"
                    export default {
                      async acquire() {
                        return {
                          url: 'https://evil.example/book.epub',
                          rightsStatus: 'public_domain',
                          mimeType: 'application/epub+zip'
                        }
                      }
                    }
                "#
                .into(),
                Arc::new(FixtureHttp),
                root.clone(),
                "runtime-test".into(),
            );
            assert!(out_of_domain
                .acquire("book-1", AcquireMode::CacheForReading)
                .unwrap_err()
                .contains("manifest 域名白名单"));

            let rights_escalation = PluginRuntime::new(
                manifest(),
                r#"
                    export default {
                      async acquire() {
                        return {
                          url: 'https://example.com/book.epub',
                          rightsStatus: 'official_free',
                          mimeType: 'application/epub+zip'
                        }
                      }
                    }
                "#
                .into(),
                Arc::new(FixtureHttp),
                root.clone(),
                "runtime-test".into(),
            );
            assert!(rights_escalation
                .acquire("book-1", AcquireMode::CacheForReading)
                .unwrap_err()
                .contains("public-domain or open-license"));
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn offline_smoke_plugin_supports_formal_source_calls() {
            let root = plugin_root();
            std::fs::create_dir_all(root.join("test-plugin-hello")).unwrap();
            let smoke_manifest = crate::plugin_manifest::parse_manifest_json(include_str!(
                "../../../scripts/test-plugin/manifest.json"
            ))
            .unwrap();
            let runtime = PluginRuntime::new(
                smoke_manifest,
                include_str!("../../../scripts/test-plugin/index.js").into(),
                Arc::new(FixtureHttp),
                root.clone(),
                "test-plugin-hello".into(),
            );

            let search = runtime.search("正式来源", 1).unwrap();
            assert_eq!(search.results.len(), 2);
            let book = runtime.get_book(&search.results[0].url).unwrap();
            assert_eq!(book.chapters.len(), 3);
            let chapter = runtime.get_chapter(&book.chapters[0].url).unwrap();
            assert!(chapter.html.contains("完整插件流程工作正常"));
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

#[cfg(not(feature = "quickjs"))]
mod imp {
    use std::path::PathBuf;
    use std::sync::Arc;

    use serde::{Deserialize, Serialize};

    use crate::plugin_host::{
        AcquireMode, AcquireProposal, HostHttpGetPlan, HostHttpResponse, PluginBookDetail,
        PluginChapterContent, PluginSearchPage,
    };
    use crate::plugin_manifest::PluginManifest;

    pub trait PluginHttpExecutor: Send + Sync {
        fn execute(&self, _plan: HostHttpGetPlan) -> Result<HostHttpResponse, String> {
            Err("QuickJS 未启用（需 quickjs feature）".into())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct PluginTestFlowResult {
        pub search: PluginSearchPage,
        pub book: PluginBookDetail,
        pub chapter: PluginChapterContent,
    }

    pub struct PluginRuntime;

    impl PluginRuntime {
        pub fn new(
            _manifest: PluginManifest,
            _entry_js: String,
            _http: Arc<dyn PluginHttpExecutor>,
            _plugin_root: PathBuf,
            _plugin_id: String,
        ) -> Self {
            Self
        }

        pub fn call(&self, _method: &str, _args_json: &str) -> Result<String, String> {
            Err("QuickJS 运行时未编译（启用 quickjs feature 以支持插件 JS 执行）".into())
        }

        pub fn search(&self, _query: &str, _page: u32) -> Result<PluginSearchPage, String> {
            Err("QuickJS 运行时未编译（启用 quickjs feature 以支持插件 JS 执行）".into())
        }

        pub fn get_book(&self, _book_url: &str) -> Result<PluginBookDetail, String> {
            Err("QuickJS 运行时未编译（启用 quickjs feature 以支持插件 JS 执行）".into())
        }

        pub fn get_chapter(&self, _chapter_url: &str) -> Result<PluginChapterContent, String> {
            Err("QuickJS 运行时未编译（启用 quickjs feature 以支持插件 JS 执行）".into())
        }

        pub fn acquire(
            &self,
            _remote_id: &str,
            _mode: AcquireMode,
        ) -> Result<AcquireProposal, String> {
            Err("QuickJS 运行时未编译（启用 quickjs feature 以支持插件 JS 执行）".into())
        }

        pub fn run_test_flow(&self, _query: &str) -> Result<PluginTestFlowResult, String> {
            Err("QuickJS 运行时未编译（启用 quickjs feature 以支持插件 JS 执行）".into())
        }
    }
}

pub use imp::*;
