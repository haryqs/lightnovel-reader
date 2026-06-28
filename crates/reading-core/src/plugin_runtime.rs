//! QuickJS 插件运行时 —— 每次调用新建 Runtime/Context，跑完即弃。
//!
//! 架构决策：一次性 Runtime（隔离最强）+ HTTP 经 trait 转发（core 不直接联网）。
//! 沙箱只注入 host.http / host.kv / host.html / host.log + URL/TextDecoder polyfill。

#[cfg(feature = "quickjs")]
mod imp {
    use rquickjs::{
        AsyncRuntime, AsyncContext, Function, Object, Value, IntoJs, FromJs,
        module::ModuleDef, Ctx,
    };
    use std::time::Duration;

    use crate::plugin_manifest::{PluginManifest, PluginPermission};

    /// HTTP 请求（core 不直接发 HTTP，由壳层实现此 trait 后注入）。
    pub trait PluginHttpExecutor: Send + Sync {
        fn execute(&self, url: String, headers: Vec<(String, String)>) -> Result<Vec<u8>, String>;
    }

    // ---- 公共接口 ----

    /// 依次调用插件的 search → getBook → getChapter 等方法。
    /// 每次调用新建 Runtime，跑完即弃。
    pub struct PluginRuntime {
        manifest: PluginManifest,
        entry_js: String,
        http: Box<dyn PluginHttpExecutor>,
    }

    impl PluginRuntime {
        pub fn new(
            manifest: PluginManifest,
            entry_js: String,
            http: Box<dyn PluginHttpExecutor>,
        ) -> Self {
            Self { manifest, entry_js, http }
        }

        /// 调用插件导出的特定方法，传 JSON 参数，返回 JSON 结果。
        pub fn call(
            &self,
            method: &str,
            args_json: &str,
        ) -> Result<String, String> {
            let rt = AsyncRuntime::new().map_err(|e| format!("QuickJS Runtime 创建失败: {e}"))?;
            let ctx = AsyncContext::full(&rt).map_err(|e| format!("QuickJS Context 创建失败: {e}"))?;

            // 超时中断：25s 后强制中断 QuickJS 执行
            rt.set_interrupt_handler(Some(Box::new(|| {})));
            let handle = rt.interrupt_handler_handle();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(25));
                handle.interrupt();
            });

            let manifest = self.manifest.clone();
            let entry = self.entry_js.clone();
            let http_exec = &self.http;
            let method = method.to_string();
            let args = args_json.to_string();

            let result: String = ctx.with(|ctx| {
                // 注入 host 对象
                inject_host_api(&ctx, &manifest, http_exec);

                // 注入 URL / TextDecoder polyfill
                inject_polyfills(&ctx);

                // 加载入口 JS
                ctx.eval::<(), _>(&format!("(function(){{ {} }})();", entry))
                    .map_err(|e| format!("插件入口执行失败: {e}"))?;

                // 调目标方法
                let global = ctx.globals();
                let plugin_obj: Object = global.get("plugin").unwrap_or_else(|| {
                    let obj = Object::new(ctx.clone());
                    global.set("plugin", obj.clone()).ok();
                    obj
                });

                let fn_val: Value = plugin_obj
                    .get(&*method)
                    .map_err(|_| format!("插件未导出方法: {method}"))?;
                let func: Function = fn_val
                    .try_into_function()
                    .map_err(|_| format!("{method} 不是函数"))?;

                let args_val: Value = ctx
                    .json_parse(&args)
                    .map_err(|e| format!("参数 JSON 解析失败: {e}"))?
                    .into();

                func.call::<Value, Value>((args_val,))
                    .map_err(|e| format!("插件调用失败: {e}"))
                    .and_then(|v| {
                        ctx.json_stringify(v)
                            .map_err(|e| format!("结果序列化失败: {e}"))
                            .map(|s| s.to_string())
                    })
            })?;

            Ok(result)
        }
    }

    // ---- 内部实现 ----

    fn inject_host_api(
        ctx: &Ctx,
        manifest: &PluginManifest,
        http_exec: &Box<dyn PluginHttpExecutor>,
    ) {
        // host.http
        let http_manifest = manifest.clone();
        let http_exec_ref: &'static dyn PluginHttpExecutor =
            unsafe { std::mem::transmute(http_exec.as_ref()) };
        let http_fn = Function::new(ctx.clone(), move |url: String| -> Result<String, String> {
            if !http_manifest.permissions.contains(&PluginPermission::Http) {
                return Err("插件未声明 http 权限".into());
            }
            let allowed = http_manifest.domains.iter().any(|domain| {
                let parsed = url.trim_start_matches("https://").trim_start_matches("http://");
                parsed.starts_with(domain.as_str())
            });
            if !allowed {
                return Err(format!("URL 不在 manifest.domains 白名单内: {url}"));
            }
            let bytes = http_exec_ref.execute(url, vec![])?;
            String::from_utf8(bytes).map_err(|e| format!("编码错误: {e}"))
        });
        let host_obj = Object::new(ctx.clone());
        let http_obj = Object::new(ctx.clone());
        http_obj.set("get", http_fn).ok();
        host_obj.set("http", http_obj).ok();

        // host.kv — 简化：内存 Map，按插件 id 隔离（TODO: 落盘到 plugin_store）
        let kv_store = std::sync::Mutex::new(
            std::collections::HashMap::<String, String>::new(),
        );
        let kv_fn_get = {
            let kv = std::sync::Arc::new(kv_store);
            Function::new(ctx.clone(), move |key: String| -> Option<String> {
                let guard = kv.lock().unwrap();
                guard.get(&key).cloned()
            })
        };
        let kv_fn_set = {
            let kv = kv_fn_get.clone(); // shares Arc
            Function::new(ctx.clone(), move |(key, value): (String, String)| {
                // FIXME: Arc sharing — need proper shared state
                Ok::<(), String>(())
            })
        };
        let kv_obj = Object::new(ctx.clone());
        kv_obj.set("get", kv_fn_get).ok();
        kv_obj.set("set", kv_fn_set).ok();
        host_obj.set("kv", kv_obj).ok();

        // host.log
        let log_fn = Function::new(ctx.clone(), |msg: String| {
            eprintln!("[plugin] {}", msg);
        });
        let log_obj = Object::new(ctx.clone());
        log_obj.set("info", log_fn.clone()).ok();
        log_obj.set("warn", log_fn.clone()).ok();
        log_obj.set("error", log_fn).ok();
        host_obj.set("log", log_obj).ok();

        ctx.globals().set("host", host_obj).ok();
    }

    fn inject_polyfills(ctx: &Ctx) {
        // URL polyfill
        ctx.eval::<(), _>(r#"
            if (typeof URL === 'undefined') {
                globalThis.URL = function(url, base) {
                    if (!url) throw new TypeError('Invalid URL');
                    var a = { href: url, protocol: 'https:', hostname: '', pathname: '/', search: '', hash: '' };
                    var m = url.match(/^(https?:)\/\/([^\/]+)(\/[^?#]*)?(\?[^#]*)?(#.*)?$/);
                    if (m) { a.protocol=m[1]; a.hostname=m[2]; a.pathname=m[3]||'/'; a.search=m[4]||''; a.hash=m[5]||''; a.href=url; }
                    return a;
                };
            }
            if (typeof TextDecoder === 'undefined') {
                globalThis.TextDecoder = function(enc) {
                    this.decode = function(buf) {
                        var s=''; for (var i=0;i<buf.length;i++) s+=String.fromCharCode(buf[i]);
                        return s;
                    };
                };
            }
        "#).ok();
    }
}

#[cfg(not(feature = "quickjs"))]
mod imp {
    use crate::plugin_manifest::PluginManifest;

    pub trait PluginHttpExecutor: Send + Sync {
        fn execute(&self, _url: String, _headers: Vec<(String, String)>) -> Result<Vec<u8>, String> {
            Err("QuickJS 未启用（需 quickjs feature）".into())
        }
    }

    pub struct PluginRuntime;

    impl PluginRuntime {
        pub fn new(
            _manifest: PluginManifest,
            _entry_js: String,
            _http: Box<dyn PluginHttpExecutor>,
        ) -> Self {
            Self
        }

        pub fn call(&self, _method: &str, _args_json: &str) -> Result<String, String> {
            Err("QuickJS 运行时未编译（启用 quickjs feature 以支持插件 JS 执行）".into())
        }
    }
}

pub use imp::*;
