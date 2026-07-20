use std::collections::HashMap;

use crate::epub_parser::ManifestItem;

pub fn clean_chapter(
    raw: &str,
    _opf_dir: &str,
    _manifest: &HashMap<String, ManifestItem>,
) -> Result<String, String> {
    let body_content = extract_body(raw);
    // 安全清洗先行：EPUB 正文是不可信输入，正文经 innerHTML 注入主文档
    // （持有 window.__TAURI__、CSP=null）。必须先剥掉脚本、事件处理属性与
    // javascript: URL，后续排版改写都在已净化的 HTML 上进行。
    let safe = sanitize_security(&body_content);
    let no_font = strip_font_tags(&safe);
    let cleaned = strip_line_heights(&no_font);
    let cleaned = strip_dangerous_styles(&cleaned);
    let imgs = rewrite_img_tags(&cleaned);
    let svgs = rewrite_svg_image_blocks(&imgs);
    let final_html = inject_typography(&svgs);
    Ok(final_html)
}

/// 安全清洗：移除可执行/危险元素、事件处理属性与脚本类 URL。
///
/// 注：这是基于字符串扫描的“足够好”防线，不是 HTML5 解析器级别的清洗。
/// 长期建议（见 DECISIONS.md）评估引入 `ammonia` 等基于真正 HTML 解析的清洗库，
/// 以抵御畸形/嵌套绕过。当前实现覆盖常见向量并有充分单测。
fn sanitize_security(html: &str) -> String {
    // 1) 整块移除可执行/危险容器元素（连同内容）
    let s = remove_container_elements(html, &["script", "iframe", "object", "applet"]);
    // 2) 移除危险空元素（embed 无闭合标签；base/link/meta 可重定向/改写基址/加载外部资源）
    let s = remove_void_elements(&s, &["embed", "base", "link", "meta"]);
    // 3) 逐标签剥事件处理属性，并中和 javascript:/vbscript: URL
    scrub_attributes(&s)
}

/// 清洗插件返回的正文 HTML 片段，不做 EPUB 路径改写与排版注入。
pub fn sanitize_fragment(html: &str) -> String {
    sanitize_security(html)
}

/// 移除 `<name ...>...</name>` 容器元素（含内容）。未闭合则删到结尾。大小写无关。
fn remove_container_elements(html: &str, names: &[&str]) -> String {
    let mut s = html.to_string();
    for name in names {
        s = remove_one_container(&s, name);
    }
    s
}

fn remove_one_container(html: &str, name: &str) -> String {
    let open = format!("<{}", name);
    let close = format!("</{}>", name);
    let mut result = String::with_capacity(html.len());
    let mut pos = 0;
    while let Some(start) = find_ci(html, &open, pos) {
        let after = start + open.len();
        // 边界校验：`<script` 命中，`<scripting` 不命中
        let boundary_ok = html[after..]
            .chars()
            .next()
            .map_or(true, |c| c.is_whitespace() || c == '>' || c == '/');
        if !boundary_ok {
            result.push_str(&html[pos..after]);
            pos = after;
            continue;
        }
        result.push_str(&html[pos..start]);
        match find_ci(html, &close, after) {
            Some(c) => pos = c + close.len(),
            None => pos = html.len(), // 未闭合 → 删到结尾
        }
    }
    result.push_str(&html[pos..]);
    result
}

/// 移除 `<name ...>` 空元素（无闭合标签）。大小写无关。
fn remove_void_elements(html: &str, names: &[&str]) -> String {
    let mut s = html.to_string();
    for name in names {
        let open = format!("<{}", name);
        let mut result = String::with_capacity(s.len());
        let mut pos = 0;
        while let Some(start) = find_ci(&s, &open, pos) {
            let after = start + open.len();
            let boundary_ok = s[after..]
                .chars()
                .next()
                .map_or(true, |c| c.is_whitespace() || c == '>' || c == '/');
            if !boundary_ok {
                result.push_str(&s[pos..after]);
                pos = after;
                continue;
            }
            result.push_str(&s[pos..start]);
            match s[after..].find('>') {
                Some(g) => pos = after + g + 1,
                None => pos = s.len(),
            }
        }
        result.push_str(&s[pos..]);
        s = result;
    }
    s
}

/// 找到从 `start`（指向 `<`）起的标签结束 `>` 的字节下标，跳过引号内的 `>`。
/// `<`、`>`、引号均为 ASCII，按字节扫描对 UTF-8 安全。
fn find_tag_end(html: &str, start: usize) -> Option<usize> {
    let b = html.as_bytes();
    let mut i = start + 1;
    let mut quote: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                b'"' | b'\'' => quote = Some(c),
                b'>' => return Some(i),
                _ => {}
            },
        }
        i += 1;
    }
    None
}

/// 逐标签扫描：剥掉所有 `on*` 事件处理属性，并中和 href/src 中的脚本类 URL。
/// 非标签文本原样保留；`</...>`、注释 `<!--`、`<?...?>` 不动。
fn scrub_attributes(html: &str) -> String {
    let b = html.as_bytes();
    let mut result = String::with_capacity(html.len());
    let mut i = 0;
    while i < html.len() {
        if b[i] == b'<' {
            let next = b.get(i + 1).copied();
            let is_tag = matches!(next, Some(c) if c.is_ascii_alphabetic() || c == b'/' || c == b'!' || c == b'?');
            if is_tag {
                if let Some(end) = find_tag_end(html, i) {
                    let inner = &html[i + 1..end];
                    if inner.starts_with('/') || inner.starts_with('!') || inner.starts_with('?') {
                        result.push_str(&html[i..=end]);
                    } else {
                        result.push_str(&scrub_one_tag(inner));
                    }
                    i = end + 1;
                    continue;
                } else {
                    result.push_str(&html[i..]);
                    break;
                }
            }
            // 不是标签（如正文里的 `a < b`），按文本输出
            result.push('<');
            i += 1;
        } else {
            // ASCII '<' 之外按 UTF-8 字符推进
            let ch = html[i..].chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
        }
    }
    result
}

/// 重建单个开始标签（`inner` 是 `<` 与 `>` 之间的内容），丢弃 on* 属性与脚本类 URL。
fn scrub_one_tag(inner: &str) -> String {
    let chars: Vec<char> = inner.chars().collect();
    let mut idx = 0;
    let n = chars.len();

    // 标签名
    let mut name = String::new();
    while idx < n && !chars[idx].is_whitespace() && chars[idx] != '/' {
        name.push(chars[idx]);
        idx += 1;
    }

    let mut out = format!("<{}", name);
    let mut self_closing = false;

    loop {
        // 跳过空白
        while idx < n && chars[idx].is_whitespace() {
            idx += 1;
        }
        if idx >= n {
            break;
        }
        if chars[idx] == '/' {
            self_closing = true;
            idx += 1;
            continue;
        }

        // 属性名
        let mut attr = String::new();
        while idx < n && !chars[idx].is_whitespace() && chars[idx] != '=' && chars[idx] != '/' {
            attr.push(chars[idx]);
            idx += 1;
        }
        if attr.is_empty() {
            idx += 1;
            continue;
        }

        // 可选 = 值
        let mut value: Option<(char, String)> = None; // (quote, value)；quote=' ' 表示无引号
        let save = idx;
        while idx < n && chars[idx].is_whitespace() {
            idx += 1;
        }
        if idx < n && chars[idx] == '=' {
            idx += 1;
            while idx < n && chars[idx].is_whitespace() {
                idx += 1;
            }
            if idx < n && (chars[idx] == '"' || chars[idx] == '\'') {
                let q = chars[idx];
                idx += 1;
                let mut v = String::new();
                while idx < n && chars[idx] != q {
                    v.push(chars[idx]);
                    idx += 1;
                }
                if idx < n {
                    idx += 1; // 跳过结束引号
                }
                value = Some((q, v));
            } else {
                let mut v = String::new();
                while idx < n && !chars[idx].is_whitespace() && chars[idx] != '/' {
                    v.push(chars[idx]);
                    idx += 1;
                }
                value = Some((' ', v));
            }
        } else {
            idx = save; // 没有 =，是布尔属性
        }

        let attr_lower = attr.to_ascii_lowercase();

        // 丢弃事件处理属性
        if attr_lower.starts_with("on") {
            continue;
        }
        // 中和脚本类 URL 属性
        if matches!(
            attr_lower.as_str(),
            "href" | "src" | "xlink:href" | "formaction" | "action"
        ) {
            if let Some((_, ref v)) = value {
                if is_script_url(v) {
                    continue; // 直接丢弃该属性
                }
            }
        }

        // 重建属性
        match value {
            None => {
                out.push(' ');
                out.push_str(&attr);
            }
            Some((q, v)) => {
                out.push(' ');
                out.push_str(&attr);
                out.push('=');
                // 值含双引号则用单引号包裹，否则统一双引号
                if q == '\'' || v.contains('"') {
                    out.push('\'');
                    out.push_str(&v.replace('\'', "&#39;"));
                    out.push('\'');
                } else {
                    out.push('"');
                    out.push_str(&v);
                    out.push('"');
                }
            }
        }
    }

    if self_closing {
        out.push_str(" /");
    }
    out.push('>');
    out
}

/// 判断 URL 值是否为脚本类协议。先解码 HTML 实体（浏览器会在属性值里解码
/// `javascript&#58;` → `javascript:`），再去空白/控制字符与大小写，最后比对前缀。
fn is_script_url(value: &str) -> bool {
    let decoded = decode_entities_for_scheme(value);
    let mut s = String::new();
    for c in decoded.chars() {
        // 去掉空白与控制字符（绕过手法如 "java\tscript:" / "&#9;"）
        if c.is_whitespace() || c.is_control() {
            continue;
        }
        s.push(c.to_ascii_lowercase());
    }
    s.starts_with("javascript:") || s.starts_with("vbscript:") || s.starts_with("data:text/html")
}

/// 解码与 scheme 检测相关的 HTML 实体：数字实体 `&#58;`/`&#x3a;`（分号可选）与
/// 命名实体 `&colon;`。其它实体保持原样——对 scheme 前缀判断够用，且字母数字实体
/// （如 `&#115;`=s）也能解出，挫败 `java&#115;cript:` 这类拆字绕过。
fn decode_entities_for_scheme(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut out = String::new();
    let mut i = 0;
    while i < n {
        if chars[i] == '&' {
            // 命名 &colon;
            let window: String = chars[i..(i + 7).min(n)].iter().collect();
            if window.eq_ignore_ascii_case("&colon;") {
                out.push(':');
                i += 7;
                continue;
            }
            // 数字实体 &#dd; / &#xhh;
            if i + 1 < n && chars[i + 1] == '#' {
                let mut j = i + 2;
                let hex = j < n && (chars[j] == 'x' || chars[j] == 'X');
                if hex {
                    j += 1;
                }
                let start = j;
                while j < n
                    && (if hex {
                        chars[j].is_ascii_hexdigit()
                    } else {
                        chars[j].is_ascii_digit()
                    })
                {
                    j += 1;
                }
                if j > start {
                    let num: String = chars[start..j].iter().collect();
                    let code = if hex {
                        u32::from_str_radix(&num, 16)
                    } else {
                        num.parse::<u32>()
                    };
                    if let Ok(cp) = code {
                        if let Some(ch) = char::from_u32(cp) {
                            out.push(ch);
                        }
                    }
                    if j < n && chars[j] == ';' {
                        j += 1;
                    }
                    i = j;
                    continue;
                }
            }
            out.push('&');
            i += 1;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// 从标签里取某属性值（支持单/双引号；`href` 也能命中 `xlink:href`）。
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let key = format!("{}=", attr);
    let pos = lower.find(&key)?;
    let after = &tag[pos + key.len()..];
    let q = after.chars().next()?;
    if q == '"' || q == '\'' {
        let rest = &after[1..];
        let end = rest.find(q)?;
        Some(rest[..end].to_string())
    } else {
        let end = after
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(after.len());
        Some(after[..end].to_string())
    }
}

fn img_html(src: &str) -> String {
    // 保留路径上下文让 reader-img 协议处理器做按需查找（basename 兜底）
    let clean = src.replace('\\', "/").trim_start_matches("./").to_string();
    // Tauri v2 自定义协议的 URL 形式因平台而异：
    // Windows(WebView2)/Android 走 http://<scheme>.localhost/，其余走 <scheme>://localhost/。
    // 用 reader-img://localhost 在 Windows 上不会解析 → 破图。
    #[cfg(any(windows, target_os = "android"))]
    let url = format!("http://reader-img.localhost/{}", clean);
    #[cfg(not(any(windows, target_os = "android")))]
    let url = format!("reader-img://localhost/{}", clean);
    format!(
        "<img src=\"{}\" style=\"max-width:100%;height:auto\"/>",
        url
    )
}

/// 在 haystack 中大小写无关地查找 ASCII needle，返回原串中的字节偏移。
/// 不用 to_lowercase()，避免多字节字符改变长度导致偏移错位。
fn find_ci(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    let nlen = needle.len();
    if nlen == 0 || haystack.len() < nlen {
        return None;
    }
    let mut start = from;
    while start + nlen <= haystack.len() {
        if let Some(slice) = haystack.get(start..start + nlen) {
            if slice.eq_ignore_ascii_case(needle) {
                return Some(start);
            }
            start += 1;
        } else {
            start += 1; // 非字符边界，前进
        }
    }
    None
}

/// 剥掉 `<svg>...</svg>` 整块，将内含的 `<image>`/`<img>` 改写为 reader-img 协议，
/// 其余直接去掉（文本阅读器不渲染矢量图）。
fn rewrite_svg_image_blocks(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut pos = 0;
    while pos < html.len() {
        match find_ci(html, "<svg", pos) {
            Some(svg_start) => {
                result.push_str(&html[pos..svg_start]);
                match find_ci(html, "</svg>", svg_start) {
                    Some(close) => {
                        let block_end = close + "</svg>".len();
                        let block = &html[svg_start..block_end];

                        let mut found_src = None;
                        if let Some(image_start) = find_ci(block, "<image", 0) {
                            let rest = &block[image_start..];
                            let image_tag_end = rest
                                .find('>')
                                .map(|p| image_start + p + 1)
                                .unwrap_or(block.len());
                            let image_tag = &block[image_start..image_tag_end.min(block.len())];
                            if let Some(src) = extract_attr(image_tag, "xlink:href")
                                .or_else(|| extract_attr(image_tag, "href"))
                            {
                                found_src = Some(src);
                            }
                        }
                        if found_src.is_none() {
                            if let Some(img_start) = find_ci(block, "<img", 0) {
                                let rest = &block[img_start..];
                                let img_tag_end = rest
                                    .find('>')
                                    .map(|p| img_start + p + 1)
                                    .unwrap_or(block.len());
                                let img_tag = &block[img_start..img_tag_end.min(block.len())];
                                if let Some(src) = extract_attr(img_tag, "src") {
                                    found_src = Some(src);
                                }
                            }
                        }

                        if let Some(src) = found_src {
                            result.push_str(&img_html(&src));
                        }

                        pos = block_end;
                    }
                    None => {
                        break;
                    }
                }
            }
            None => {
                result.push_str(&html[pos..]);
                break;
            }
        }
    }
    result
}

fn extract_body(raw: &str) -> String {
    let lower = raw.to_lowercase();
    let body_start = lower.find("<body");

    if let Some(start) = body_start {
        // 跳过 <body ...> 开标签
        let body_tag_end = lower[start..]
            .find('>')
            .map(|i| start + i + 1)
            .unwrap_or(start);

        let body_end = lower[body_tag_end..].find("</body>");

        if let Some(end) = body_end {
            let end_pos = body_tag_end + end;
            if end_pos > body_tag_end {
                return raw[body_tag_end..end_pos].to_string();
            }
        }

        // 找不到 </body>，返回 body 之后全部内容
        if body_tag_end < raw.len() {
            return raw[body_tag_end..].to_string();
        }
    }

    raw.to_string()
}

fn strip_dangerous_styles(html: &str) -> String {
    let dangerous = [
        "position",
        "display",
        "float",
        "clear",
        "z-index",
        "overflow",
        "visibility",
        "transform",
        "animation",
        "transition",
    ];

    let mut result = String::with_capacity(html.len());
    let chars: Vec<char> = html.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // 检测 style="
        if i + 6 < chars.len() && chars[i..i + 7].iter().collect::<String>() == "style=\"" {
            result.push_str("style=\"");
            let start = i + 7;
            let mut j = start;
            let mut style_content = String::new();
            while j < chars.len() && chars[j] != '"' {
                style_content.push(chars[j]);
                j += 1;
            }

            let filtered: Vec<String> = style_content
                .split(';')
                .filter(|decl| {
                    let decl_lower = decl.trim().to_lowercase();
                    !dangerous.iter().any(|prop| decl_lower.starts_with(prop))
                })
                .filter(|decl| !decl.trim().is_empty())
                .map(|d| d.trim().to_string())
                .collect();

            result.push_str(&filtered.join("; "));
            if !filtered.is_empty() {
                result.push(';');
            }
            result.push('"');

            i = j; // j 指向 " 字符，下次循环 ++ 跳过
        } else {
            result.push(chars[i]);
        }
        i += 1;
    }

    result
}

/// 去除 &lt;font&gt; 标签，将其 size/color 属性转为内联 style 的 &lt;span&gt;。
/// 例: &lt;font size="5" color="red"&gt;文字&lt;/font&gt; → &lt;span style="font-size:large;color:red"&gt;文字&lt;/span&gt;
fn strip_font_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let chars: Vec<(usize, char)> = html.char_indices().collect();
    let mut ci = 0;

    while ci < chars.len() {
        let (byte_pos, _) = chars[ci];
        let remaining = &html[byte_pos..];

        if remaining.to_lowercase().starts_with("<font") {
            let tag_end = remaining
                .find('>')
                .map(|p| p + 1)
                .unwrap_or(remaining.len());
            let font_tag = &remaining[..tag_end];

            // 提取 size 和 color
            let size_attr = extract_attr(font_tag, "size");
            let color_attr = extract_attr(font_tag, "color");

            let mut styles = Vec::new();
            if let Some(ref s) = size_attr {
                let fs = match s.as_str() {
                    "1" => "x-small",
                    "2" => "small",
                    "3" => "medium",
                    "4" => "large",
                    "5" => "x-large",
                    "6" => "xx-large",
                    "7" => "xxx-large",
                    _ => s,
                };
                styles.push(format!("font-size:{}", fs));
            }
            if let Some(ref c) = color_attr {
                styles.push(format!("color:{}", c));
            }

            if styles.is_empty() {
                result.push_str("<span>");
            } else {
                result.push_str(&format!("<span style=\"{}\">", styles.join(";")));
            }

            let tag_byte_len = tag_end;
            let tag_end_byte = byte_pos + tag_byte_len;
            while ci < chars.len() && chars[ci].0 < tag_end_byte {
                ci += 1;
            }
        } else if remaining.to_lowercase().starts_with("</font") {
            result.push_str("</span>");
            let tag_len = remaining.find('>').map(|p| p + 1).unwrap_or(7);
            let tag_end_byte = byte_pos + tag_len;
            while ci < chars.len() && chars[ci].0 < tag_end_byte {
                ci += 1;
            }
        } else {
            result.push(chars[ci].1);
            ci += 1;
        }
    }
    result
}

/// 删除出版方在 body/p/div 上的 line-height，为统一排版让路。
fn strip_line_heights(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let chars: Vec<char> = html.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if i + 6 < chars.len() && chars[i..i + 7].iter().collect::<String>() == "style=\"" {
            result.push_str("style=\"");
            let start = i + 7;
            let mut j = start;
            let mut style_content = String::new();
            while j < chars.len() && chars[j] != '"' {
                style_content.push(chars[j]);
                j += 1;
            }

            let filtered: Vec<String> = style_content
                .split(';')
                .filter(|decl| {
                    let d = decl.trim().to_lowercase();
                    !d.starts_with("line-height")
                })
                .filter(|decl| !decl.trim().is_empty())
                .map(|d| d.trim().to_string())
                .collect();

            if filtered.is_empty() {
                // 去掉空的 style=""
                result.pop(); // 去掉我们推的 style="
                              // result 最后 7 个字符是 "style=""，全回退
                result.truncate(result.len().saturating_sub(7));
                i = j + 1;
            } else {
                result.push_str(&filtered.join("; "));
                if !filtered.is_empty() {
                    result.push(';');
                }
                result.push('"');
                i = j;
            }
        } else {
            result.push(chars[i]);
        }
        i += 1;
    }
    result
}

/// 注入统一排版基线——行高 1.8、段落间距、首行缩进。
/// 在 &lt;/head&gt; 之前插入 &lt;style&gt; 块，优先级低于用户主题。
fn inject_typography(html: &str) -> String {
    let baseline_css = r#"
<style data-source="reader-typography">
  body, p, div, li, td, th, blockquote {
    line-height: 1.8 !important;
  }
  p {
    margin-bottom: 0.8em;
    text-indent: 2em;
  }
  h1, h2, h3, h4, h5, h6 {
    line-height: 1.4 !important;
  }
</style>"#;

    if let Some(head_close) = html.to_lowercase().find("</head>") {
        let mut result = String::with_capacity(html.len() + baseline_css.len());
        result.push_str(&html[..head_close]);
        result.push_str(baseline_css);
        result.push_str(&html[head_close..]);
        result
    } else {
        // 没有 head 标签，插到最前面
        format!("{}{}", baseline_css, html)
    }
}

fn rewrite_img_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let chars: Vec<(usize, char)> = html.char_indices().collect();
    let mut ci = 0;

    while ci < chars.len() {
        let (byte_pos, _) = chars[ci];
        let remaining = &html[byte_pos..];
        if remaining.to_lowercase().starts_with("<img") {
            let tag_end_off = remaining
                .find('>')
                .map(|p| p + 1)
                .unwrap_or(remaining.len());
            let img_tag = &remaining[..tag_end_off];

            if let Some(src) = extract_attr(img_tag, "src") {
                result.push_str(&img_html(&src));
            }

            let tag_byte_len = tag_end_off;
            let tag_end_byte = byte_pos + tag_byte_len;
            while ci < chars.len() && chars[ci].0 < tag_end_byte {
                ci += 1;
            }
        } else {
            result.push(chars[ci].1);
            ci += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_font_tag_to_span() {
        let html = r#"<font size="5" color="red">大字红色</font>"#;
        let out = strip_font_tags(html);
        assert!(out.contains("<span"), "应转为 span");
        assert!(out.contains("font-size:x-large"), "size=5 -> x-large");
        assert!(out.contains("color:red"), "color 应保留");
        assert!(
            !out.to_lowercase().contains("<font"),
            "不应有 font 标签残留"
        );
    }

    #[test]
    fn strip_font_without_attrs() {
        let html = "<font>普通文字</font>";
        let out = strip_font_tags(html);
        assert!(out.contains("<span>普通文字</span>"), "无属性 font 转 span");
    }

    #[test]
    fn strip_line_height_removes_lh() {
        let html = r#"<p style="line-height: 1.2; color: #333">p</p>"#;
        let out = strip_line_heights(html);
        assert!(!out.contains("line-height"), "line-height 应被移除");
        assert!(out.contains("color"), "color 应保留");
    }

    #[test]
    fn inject_typography_adds_baseline() {
        let html = "<html><head><title>X</title></head><body><p>hi</p></body></html>";
        let out = inject_typography(html);
        assert!(out.contains("reader-typography"), "应注入排版");
        assert!(out.contains("line-height: 1.8"), "应有行高 1.8");
        assert!(out.contains("text-indent: 2em"), "应有首行缩进");
    }

    #[test]
    fn full_pipeline_cleanup() {
        let raw = "<html><head></head><body><p style='line-height:1.2'>p</p><font size='4' color='blue'>x</font></body></html>";
        let result = clean_chapter(raw, "", &HashMap::new()).unwrap();
        assert!(!result.to_lowercase().contains("<font"), "font 标签已清理");
        assert!(result.contains("reader-typography"), "排版已注入");
    }

    #[test]
    fn rewrites_svg_image_block_to_reader_img() {
        let html = r#"<p>前文</p><svg viewBox="0 0 600 800" xmlns="http://www.w3.org/2000/svg"><image width="600" height="800" xlink:href="../Images/cover.jpg"/></svg><p>后文</p>"#;
        let out = rewrite_svg_image_blocks(html);
        // URL 格式因平台而异：Windows http://reader-img.localhost/，其他 reader-img://localhost/
        assert!(
            out.contains("reader-img") && out.contains("cover.jpg"),
            "svg 内图片应改写"
        );
        assert!(!out.to_lowercase().contains("<svg"), "svg 块应被移除");
        assert!(
            out.contains("前文") && out.contains("后文"),
            "svg 前后正文保留"
        );
    }

    #[test]
    fn rewrites_svg_with_chinese_around() {
        let html = "第一章。<SVG><IMAGE href=\"a.png\"/></SVG>正文内容在这里。";
        let out = rewrite_svg_image_blocks(html);
        assert!(
            out.contains("reader-img") && out.contains("a.png"),
            "svg 内图片应改写"
        );
        assert!(!out.to_lowercase().contains("<svg"), "svg 块应被移除");
    }

    // ===== 安全清洗 =====

    #[test]
    fn removes_script_blocks_with_content() {
        let html =
            r#"<p>前</p><script>window.__TAURI__.core.invoke('library_list')</script><p>后</p>"#;
        let out = sanitize_security(html);
        assert!(!out.to_lowercase().contains("<script"), "script 标签应移除");
        assert!(!out.contains("__TAURI__"), "script 内容应一并移除");
        assert!(out.contains("前") && out.contains("后"), "正文保留");
    }

    #[test]
    fn removes_unclosed_script_to_end() {
        let html = r#"<p>正文</p><script>evil()"#;
        let out = sanitize_security(html);
        assert!(!out.to_lowercase().contains("<script"));
        assert!(!out.contains("evil()"));
        assert!(out.contains("正文"));
    }

    #[test]
    fn strips_event_handler_attributes() {
        let html = r#"<p onclick="steal()" class="x">文</p><div onmouseover="evil()">块</div>"#;
        let out = sanitize_security(html);
        assert!(!out.to_lowercase().contains("onclick"), "onclick 应剥除");
        assert!(
            !out.to_lowercase().contains("onmouseover"),
            "onmouseover 应剥除"
        );
        assert!(out.contains(r#"class="x""#), "正常属性保留");
        assert!(out.contains("文") && out.contains("块"));
    }

    #[test]
    fn strips_onerror_on_arbitrary_elements() {
        // img 会被后续重建，但 details/video 等元素的 onerror/ontoggle 也必须剥除
        let html = r#"<details ontoggle="invoke()"><summary>S</summary>X</details><video onerror="hack()"></video>"#;
        let out = sanitize_security(html);
        assert!(!out.to_lowercase().contains("ontoggle"));
        assert!(!out.to_lowercase().contains("onerror"));
        assert!(
            out.contains("<details") && out.contains("<summary"),
            "元素本身保留"
        );
    }

    #[test]
    fn neutralizes_javascript_href() {
        let html = r#"<a href="javascript:alert(1)">点</a><a href="Text/ch2.xhtml">正常</a>"#;
        let out = sanitize_security(html);
        assert!(
            !out.to_lowercase().contains("javascript:"),
            "js: 链接应去除"
        );
        assert!(out.contains(r#"href="Text/ch2.xhtml""#), "正常链接保留");
        assert!(out.contains("点") && out.contains("正常"));
    }

    #[test]
    fn neutralizes_obfuscated_js_url() {
        // 大小写 + 内嵌空白/制表符的绕过手法
        let html = "<a href=\"Java\tScript:evil()\">x</a>";
        let out = sanitize_security(html);
        assert!(!out
            .to_lowercase()
            .replace(char::is_whitespace, "")
            .contains("javascript:"));
    }

    #[test]
    fn removes_iframe_object_embed_meta() {
        let html = r#"<iframe src="http://evil"></iframe><object data="x"></object><embed src="y"><meta http-equiv="refresh" content="0;url=http://evil"><p>正文</p>"#;
        let out = sanitize_security(html);
        for bad in ["<iframe", "<object", "<embed", "<meta"] {
            assert!(!out.to_lowercase().contains(bad), "{} 应移除", bad);
        }
        assert!(out.contains("正文"));
    }

    #[test]
    fn preserves_plain_text_with_angle_brackets() {
        // 正文里的裸 `<`（非标签）不应被当作标签吞掉
        let html = "若 a < b 且 b > c 则成立";
        let out = sanitize_security(html);
        assert!(out.contains("a < b"), "数学比较文本应保留: {}", out);
    }

    #[test]
    fn keeps_cjk_and_attributes_intact() {
        let html = r#"<p style="color:red" data-k="值">中文内容</p>"#;
        let out = sanitize_security(html);
        assert!(out.contains("中文内容"));
        assert!(out.contains(r#"style="color:red""#));
        assert!(out.contains(r#"data-k="值""#));
    }

    #[test]
    fn neutralizes_entity_encoded_js_url() {
        // 浏览器会在属性值里解码实体，故必须先解码再判 scheme。
        let cases = [
            r#"<a href="javascript&#58;alert(1)">x</a>"#, // &#58; = ':'
            r#"<a href="javascript&#x3a;alert(1)">x</a>"#, // &#x3a; = ':'
            r#"<a href="java&#115;cript:alert(1)">x</a>"#, // &#115; = 's'，拆字
            r#"<a href="javascript&colon;alert(1)">x</a>"#, // 命名实体
            r#"<a href="&#106;avascript:alert(1)">x</a>"#, // &#106; = 'j'
        ];
        for c in cases {
            let out = sanitize_security(c);
            assert!(
                !out.contains("alert(1)") || !out.to_lowercase().contains("href"),
                "实体编码的 js: 应被去除: {} -> {}",
                c,
                out
            );
            assert!(
                !out.contains(r#"href="javascript"#) && !out.contains("&#58"),
                "不应残留可解码为 javascript: 的 href: {}",
                out
            );
        }
    }

    #[test]
    fn keeps_entity_in_normal_text() {
        // 正文里的实体（非 URL 属性）不受影响
        let html = "<p>版权所有 &copy; 2026，A &amp; B</p>";
        let out = sanitize_security(html);
        assert!(
            out.contains("&copy;") && out.contains("&amp;"),
            "正文实体保留: {}",
            out
        );
    }

    #[test]
    fn full_pipeline_strips_script_and_handlers() {
        let raw = r#"<html><head><script>head_evil()</script></head><body><p onclick="evil()">正文</p><script>body_evil()</script></body></html>"#;
        let out = clean_chapter(raw, "", &HashMap::new()).unwrap();
        assert!(
            !out.contains("head_evil"),
            "head script 由 extract_body 去除"
        );
        assert!(!out.contains("body_evil"), "body script 必须由安全清洗去除");
        assert!(!out.to_lowercase().contains("onclick"), "事件处理属性去除");
        assert!(out.contains("正文"));
        assert!(out.contains("reader-typography"), "排版仍注入");
    }
}
