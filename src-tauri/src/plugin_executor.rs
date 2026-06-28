//! PluginHttpExecutor 壳层实现 —— 用 reqwest 转发插件的 HTTP 请求。
//!
//! 保持 reading-core "无网络"纪律：core 只定义 trait，壳层负责实现。
//! TODO: 在 plugin_runtime 集成时从 plugin_executor 模块导入使用。

#![allow(dead_code)]

use reading_core::plugin_runtime::PluginHttpExecutor;

pub struct ReqwestExecutor;

impl PluginHttpExecutor for ReqwestExecutor {
    fn execute(&self, url: String, headers: Vec<(String, String)>) -> Result<Vec<u8>, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(25))
            .build()
            .map_err(|e| format!("HTTP 客户端创建失败: {e}"))?;

        let mut req = client.get(&url);
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req.send().map_err(|e| format!("HTTP 请求失败: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("HTTP {status}"));
        }
        resp.bytes()
            .map(|b| b.to_vec())
            .map_err(|e| format!("读取响应失败: {e}"))
    }
}
