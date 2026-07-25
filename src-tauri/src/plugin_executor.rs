//! PluginHttpExecutor 壳层实现 —— 用 reqwest 转发插件的 HTTP 请求。
//!
//! 保持 reading-core "无网络"纪律：core 只定义 trait，壳层负责实现。
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use reading_core::plugin_host::{
    HostHttpGetPlan, HostHttpResponse, MAX_PLUGIN_HTTP_RESPONSE_BYTES,
};
use reading_core::plugin_runtime::PluginHttpExecutor;

pub const PLUGIN_DOMAIN_MIN_INTERVAL: Duration = Duration::from_secs(1);

/// App-wide exact-domain scheduler. Runtimes are disposable, so the limiter must live in
/// AppState and be shared by every source command instead of being recreated per JS call.
#[derive(Debug)]
struct DomainRateLimiter {
    min_interval: Duration,
    next_slots: Mutex<HashMap<String, Instant>>,
}

impl DomainRateLimiter {
    fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            next_slots: Mutex::new(HashMap::new()),
        }
    }

    fn reserve_at(&self, host: &str, now: Instant) -> Duration {
        let mut slots = self
            .next_slots
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let slot = slots.get(host).copied().unwrap_or(now).max(now);
        slots.insert(host.to_string(), slot + self.min_interval);
        slot.saturating_duration_since(now)
    }

    fn wait(&self, host: &str) {
        let wait = self.reserve_at(&host.to_ascii_lowercase(), Instant::now());
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
    }
}

#[derive(Debug)]
pub struct ReqwestExecutor {
    limiter: DomainRateLimiter,
}

impl Default for ReqwestExecutor {
    fn default() -> Self {
        Self {
            limiter: DomainRateLimiter::new(PLUGIN_DOMAIN_MIN_INTERVAL),
        }
    }
}

impl PluginHttpExecutor for ReqwestExecutor {
    fn execute(&self, plan: HostHttpGetPlan) -> Result<HostHttpResponse, String> {
        let url = reqwest::Url::parse(&plan.url).map_err(|e| format!("HTTP URL 无效: {e}"))?;
        let host = url
            .host_str()
            .ok_or_else(|| "HTTP URL 缺少域名".to_string())?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| "HTTP URL 缺少端口".to_string())?;
        self.limiter.wait(host);
        let addresses = resolve_public_addresses(host, port)?;
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(plan.timeout_ms))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .user_agent(
                "LightNovelReader/0.3.1 source-plugin-host \
                 (+https://github.com/haryqs/lightnovel-reader)",
            )
            .resolve_to_addrs(host, &addresses)
            .build()
            .map_err(|e| format!("HTTP 客户端创建失败: {e}"))?;

        let mut req = client.get(&plan.url);
        for (k, v) in &plan.headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req.send().map_err(|e| format!("HTTP 请求失败: {e}"))?;
        let status = resp.status().as_u16();
        let headers = resp
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_string(), value.to_string()))
            })
            .collect::<BTreeMap<_, _>>();
        if resp
            .content_length()
            .is_some_and(|length| length > MAX_PLUGIN_HTTP_RESPONSE_BYTES as u64)
        {
            return Err(format!(
                "HTTP 响应超过 {} 字节上限",
                MAX_PLUGIN_HTTP_RESPONSE_BYTES
            ));
        }
        let mut body = Vec::new();
        resp.take(MAX_PLUGIN_HTTP_RESPONSE_BYTES as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|e| format!("读取响应失败: {e}"))?;
        if body.len() > MAX_PLUGIN_HTTP_RESPONSE_BYTES {
            return Err(format!(
                "HTTP 响应超过 {} 字节上限",
                MAX_PLUGIN_HTTP_RESPONSE_BYTES
            ));
        }
        Ok(HostHttpResponse {
            status,
            headers,
            body,
        })
    }
}

fn resolve_public_addresses(host: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
    let addresses = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        (host, port)
            .to_socket_addrs()
            .map_err(|e| format!("HTTP 域名解析失败: {e}"))?
            .collect::<Vec<_>>()
    };
    if addresses.is_empty() {
        return Err("HTTP 域名没有可用地址".into());
    }
    if addresses.iter().any(|addr| !is_public_ip(addr.ip())) {
        return Err("插件 HTTP 禁止访问本机或内网地址".into());
    }
    Ok(addresses)
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, _, _] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_multicast()
                || ip.is_unspecified()
                || a == 0
                || a >= 240
                || (a == 100 && (64..=127).contains(&b))
                || (a == 192 && b == 0)
                || (a == 198 && (b == 18 || b == 19)))
        }
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return is_public_ip(IpAddr::V4(mapped));
            }
            let segments = ip.segments();
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || (segments[0] & 0xfe00) == 0xfc00
                || (segments[0] & 0xffc0) == 0xfe80
                || (segments[0] == 0x2001 && segments[1] == 0x0db8))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_public_ip, DomainRateLimiter, ReqwestExecutor};
    use reading_core::plugin_host::{AcquireMode, HostHttpGetPlan, HostHttpResponse};
    use reading_core::plugin_manifest::parse_manifest_json;
    use reading_core::plugin_runtime::{PluginHttpExecutor, PluginRuntime};
    use std::collections::BTreeMap;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    struct GutenbergFixtureHttp;

    impl PluginHttpExecutor for GutenbergFixtureHttp {
        fn execute(&self, plan: HostHttpGetPlan) -> Result<HostHttpResponse, String> {
            let (content_type, body) = if plan.url.contains("/ebooks/search.opds/") {
                (
                    "application/atom+xml; charset=UTF-8",
                    r#"
                        <feed>
                          <entry>
                            <id>https://www.gutenberg.org/ebooks/subjects/search.opds/?query=Alice</id>
                            <title>Subjects matching Alice</title>
                          </entry>
                          <entry>
                            <id>https://www.gutenberg.org/ebooks/11.opds</id>
                            <title>Alice's Adventures in Wonderland</title>
                            <author><name>Carroll, Lewis</name></author>
                          </entry>
                          <link rel="next" href="/ebooks/search.opds/?query=Alice&amp;start_index=26"/>
                        </feed>
                    "#,
                )
            } else if plan.url == "https://www.gutenberg.org/ebooks/11" {
                (
                    "text/html; charset=UTF-8",
                    r#"
                        <html><body>
                          <h1>Alice's Adventures in Wonderland</h1>
                          <table class="bibrec"><tr><th>Author</th><td>Carroll, Lewis</td></tr></table>
                          <a href="/cache/epub/11/pg11-images.html">Read online</a>
                          <a href="/ebooks/11.epub3.images">EPUB3</a>
                        </body></html>
                    "#,
                )
            } else if plan.url == "https://www.gutenberg.org/cache/epub/11/pg11-images.html" {
                (
                    "text/html; charset=UTF-8",
                    r#"<html><body><h1>Chapter I</h1><p>Down the Rabbit-Hole.</p></body></html>"#,
                )
            } else {
                return Err(format!("unexpected Gutenberg fixture URL: {}", plan.url));
            };
            Ok(HostHttpResponse {
                status: 200,
                headers: BTreeMap::from([("content-type".into(), content_type.into())]),
                body: body.as_bytes().to_vec(),
            })
        }
    }

    fn gutenberg_runtime(http: Arc<dyn PluginHttpExecutor>) -> PluginRuntime {
        let manifest = parse_manifest_json(include_str!(
            "../../plugin-sdk/examples/gutenberg-test/manifest.json"
        ))
        .expect("Gutenberg manifest should remain valid");
        PluginRuntime::new(
            manifest,
            include_str!("../../plugin-sdk/examples/gutenberg-test/plugin.js").into(),
            http,
            PathBuf::new(),
            "gutenberg-test".into(),
        )
    }

    #[test]
    fn rate_limiter_is_shared_per_exact_domain() {
        let limiter = DomainRateLimiter::new(Duration::from_secs(1));
        let now = Instant::now();
        assert_eq!(limiter.reserve_at("example.com", now), Duration::ZERO);
        assert_eq!(
            limiter.reserve_at("example.com", now),
            Duration::from_secs(1)
        );
        assert_eq!(limiter.reserve_at("other.example", now), Duration::ZERO);
        assert_eq!(
            limiter.reserve_at("example.com", now + Duration::from_secs(3)),
            Duration::ZERO
        );
    }

    #[test]
    fn rejects_local_and_reserved_targets() {
        for ip in [
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1)),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            "fc00::1".parse().unwrap(),
            "fe80::1".parse().unwrap(),
        ] {
            assert!(!is_public_ip(ip), "{ip} should be rejected");
        }
    }

    #[test]
    fn accepts_public_targets() {
        for ip in [
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
            "2606:4700:4700::1111".parse().unwrap(),
        ] {
            assert!(is_public_ip(ip), "{ip} should be accepted");
        }
    }

    #[test]
    fn parses_gutenberg_opds_fixture_without_network() {
        let runtime = gutenberg_runtime(Arc::new(GutenbergFixtureHttp));
        let result = runtime
            .run_test_flow("Alice in Wonderland")
            .expect("offline Gutenberg plugin flow should pass");

        assert_eq!(result.search.results.len(), 1);
        assert_eq!(
            result.search.results[0].url,
            "https://www.gutenberg.org/ebooks/11"
        );
        assert!(result.search.has_more);
        assert_eq!(result.book.author.as_deref(), Some("Carroll, Lewis"));
        assert_eq!(result.book.chapters.len(), 1);
        assert!(result.chapter.html.contains("Down the Rabbit-Hole"));

        let proposal = runtime
            .acquire(&result.book.url, AcquireMode::CacheForReading)
            .expect("offline Gutenberg acquire proposal should pass");
        assert_eq!(
            proposal.url,
            "https://www.gutenberg.org/ebooks/11.epub3.images"
        );
        assert_eq!(proposal.mime_type.as_deref(), Some("application/epub+zip"));
    }

    #[test]
    #[ignore = "requires live Project Gutenberg network access"]
    fn runs_gutenberg_search_book_chapter_acquire_flow() {
        let runtime = gutenberg_runtime(Arc::new(ReqwestExecutor::default()));

        let result = runtime
            .run_test_flow("Alice in Wonderland")
            .expect("live Gutenberg plugin flow should pass");
        assert!(!result.search.results.is_empty());
        assert!(!result.book.chapters.is_empty());
        assert!(!result.chapter.html.is_empty());
        let proposal = runtime
            .acquire(&result.book.url, AcquireMode::CacheForReading)
            .expect("live Gutenberg acquire proposal should pass");
        assert_eq!(proposal.mime_type.as_deref(), Some("application/epub+zip"));
        assert!(proposal.url.contains("gutenberg.org"));
    }
}
