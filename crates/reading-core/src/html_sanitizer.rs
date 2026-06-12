use std::collections::HashMap;

use crate::epub_parser::ManifestItem;

pub fn clean_chapter(
    raw: &str,
    _opf_dir: &str,
    _manifest: &HashMap<String, ManifestItem>,
) -> Result<String, String> {
    let body_content = extract_body(raw);
    let no_font = strip_font_tags(&body_content);
    let cleaned = strip_line_heights(&no_font);
    let cleaned = strip_dangerous_styles(&cleaned);
    let imgs = rewrite_img_tags(&cleaned);
    let svgs = rewrite_svg_image_blocks(&imgs);
    let final_html = inject_typography(&svgs);
    Ok(final_html)
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
    format!("<img src=\"{}\" style=\"max-width:100%;height:auto\"/>", url)
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
                            let image_tag_end = rest.find('>').map(|p| image_start + p + 1).unwrap_or(block.len());
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
                                let img_tag_end = rest.find('>').map(|p| img_start + p + 1).unwrap_or(block.len());
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
        "position", "display", "float", "clear", "z-index", "overflow",
        "visibility", "transform", "animation", "transition",
    ];

    let mut result = String::with_capacity(html.len());
    let chars: Vec<char> = html.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // 检测 style="
        if i + 6 < chars.len()
            && chars[i..i + 7].iter().collect::<String>() == "style=\""
        {
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
            let tag_end = remaining.find('>').map(|p| p + 1).unwrap_or(remaining.len());
            let font_tag = &remaining[..tag_end];

            // 提取 size 和 color
            let size_attr = extract_attr(font_tag, "size");
            let color_attr = extract_attr(font_tag, "color");

            let mut styles = Vec::new();
            if let Some(ref s) = size_attr {
                let fs = match s.as_str() {
                    "1" => "x-small", "2" => "small", "3" => "medium",
                    "4" => "large", "5" => "x-large", "6" => "xx-large", "7" => "xxx-large",
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
        if i + 6 < chars.len()
            && chars[i..i + 7].iter().collect::<String>() == "style=\""
        {
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
            let tag_end_off = remaining.find('>').map(|p| p + 1).unwrap_or(remaining.len());
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
        assert!(!out.to_lowercase().contains("<font"), "不应有 font 标签残留");
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
        assert!(out.contains("reader-img") && out.contains("cover.jpg"), "svg 内图片应改写");
        assert!(!out.to_lowercase().contains("<svg"), "svg 块应被移除");
        assert!(out.contains("前文") && out.contains("后文"), "svg 前后正文保留");
    }

    #[test]
    fn rewrites_svg_with_chinese_around() {
        let html = "第一章。<SVG><IMAGE href=\"a.png\"/></SVG>正文内容在这里。";
        let out = rewrite_svg_image_blocks(html);
        assert!(out.contains("reader-img") && out.contains("a.png"), "svg 内图片应改写");
        assert!(!out.to_lowercase().contains("<svg"), "svg 块应被移除");
    }
}
