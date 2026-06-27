//! 虚拟分页 —— 把章节 HTML 按视口容量切分成"页"。
//!
//! 算法从 `src/reader-core.ts` 的 `buildVirtualPages` 原样移植，
//! 保持标点断句规则和容量估算逻辑完全一致。
//!
//! 本模块不依赖任何 I/O，native 和 wasm feature 都可编译。

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

/// 把章节 HTML 按 capacity（字符估算）切分成页。
///
/// `capacity` 应由调用方根据视口宽高/字号/版式参数计算
/// （见 `reader-core.ts::getEstimatedPageCapacity`）。
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn paginate(html: &str, capacity: usize) -> Vec<String> {
    let cap = capacity.max(180);
    let dom = match tl::parse(html, tl::ParserOptions::default()) {
        Ok(d) => d,
        Err(_) => return vec![String::new()],
    };

    let parser = dom.parser();
    let children: &[tl::NodeHandle] = dom.children();

    let blocks: Vec<Block> = children
        .iter()
        .filter_map(|handle| match handle.get(parser) {
            Some(tl::Node::Tag(tag)) => Some(element_to_page_blocks(tag, cap, parser)),
            _ => None,
        })
        .flatten()
        .collect();

    if blocks.is_empty() {
        let text: String = children
            .iter()
            .filter_map(|h| h.get(parser))
            .filter_map(|n| match n {
                tl::Node::Raw(raw) => Some(raw.as_utf8_str().to_string()),
                tl::Node::Tag(tag) => Some(inner_text_trimmed_from_tag(tag, parser)),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");
        let trimmed = text.trim().to_string();
        return if trimmed.is_empty() {
            vec![String::new()]
        } else {
            vec![format!("<p>{}</p>", escape_html(&trimmed))]
        };
    }

    let mut pages: Vec<String> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut used: usize = 0;

    for block in blocks {
        let cost = estimate_block_cost(&block.html, block.text_len, cap);
        if !current.is_empty() && used + cost > cap {
            pages.push(current.join(""));
            current = Vec::new();
            used = 0;
        }
        current.push(block.html);
        used += cost;
    }

    if !current.is_empty() {
        pages.push(current.join(""));
    }
    if pages.is_empty() {
        pages.push(String::new());
    }
    pages
}

struct Block {
    html: String,
    text_len: usize,
}

fn element_to_page_blocks(tag: &tl::HTMLTag, capacity: usize, parser: &tl::Parser) -> Vec<Block> {
    let html = tag.outer_html(parser).to_string();
    let text = inner_text_trimmed_from_tag(tag, parser);
    let text_len = text.chars().count();
    let tag_name = tag.name().as_utf8_str().to_ascii_lowercase();
    let has_complex = has_complex_content(tag, parser);
    let child_count = tag.children().all(parser).len();

    let can_split = matches!(tag_name.as_str(), "p" | "div" | "li" | "blockquote")
        && !has_complex
        && child_count <= 2
        && text_len as f64 > capacity as f64 * 1.15;

    if !can_split {
        return vec![Block { html, text_len }];
    }

    let target = capacity.max(220) * 78 / 100;
    let chunks = split_text(&text, target);
    let mut blocks = Vec::with_capacity(chunks.len());
    for (i, chunk) in chunks.into_iter().enumerate() {
        let wrapped = wrap_text_like_element(tag, parser, &chunk, i == 0);
        blocks.push(Block {
            text_len: chunk.chars().count(),
            html: wrapped,
        });
    }
    blocks
}

fn inner_text_trimmed_from_tag(tag: &tl::HTMLTag, parser: &tl::Parser) -> String {
    tag.children()
        .all(parser)
        .iter()
        .filter_map(|n| match n {
            tl::Node::Raw(raw) => Some(raw.as_utf8_str().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn has_complex_content(tag: &tl::HTMLTag, parser: &tl::Parser) -> bool {
    for node in tag.children().all(parser) {
        if let tl::Node::Tag(child_tag) = node {
            let name = child_tag.name().as_utf8_str().to_ascii_lowercase();
            if matches!(
                name.as_str(),
                "img" | "svg" | "table" | "math" | "video" | "audio" | "canvas"
            ) {
                return true;
            }
            if has_complex_content(&child_tag, parser) {
                return true;
            }
        }
    }
    false
}

/// 按标点优先断句。
fn split_text(text: &str, target: usize) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    let mut rest = text.trim().to_string();

    while rest.chars().count() > target {
        let total = rest.chars().count();
        let start = (target as f64 * 0.72) as usize;
        let end = total.min((target as f64 * 1.12) as usize);

        let window: String = rest.chars().skip(start).take(end - start).collect();
        let punct_positions: Vec<usize> = ['。', '！', '？', ';', '；']
            .iter()
            .filter_map(|&p| window.rfind(p))
            .collect();

        let cut = if let Some(&max_pos) = punct_positions.iter().max() {
            start + max_pos + 1
        } else {
            target
        };

        let cut = cut.max((target as f64 * 0.55) as usize);

        let chunk: String = rest.chars().take(cut).collect();
        chunks.push(chunk.trim().to_string());
        rest = rest
            .chars()
            .skip(cut)
            .collect::<String>()
            .trim()
            .to_string();
    }

    if !rest.is_empty() {
        chunks.push(rest);
    }
    chunks
}

fn estimate_block_cost(html: &str, text_len: usize, capacity: usize) -> usize {
    let lower = html.to_ascii_lowercase();
    let img_count = lower.matches("<img").count();
    let img_cost = img_count * ((capacity as f64 * 0.62) as usize);

    let heading_cost = if lower.trim_start().starts_with("<h1")
        || lower.trim_start().starts_with("<h2")
        || lower.trim_start().starts_with("<h3")
        || lower.trim_start().starts_with("<h4")
        || lower.trim_start().starts_with("<h5")
        || lower.trim_start().starts_with("<h6")
    {
        90
    } else {
        0
    };

    let structural_cost = if lower.contains("<table")
        || lower.contains("<svg")
        || lower.contains("<pre")
        || lower.contains("<blockquote")
    {
        160
    } else {
        28
    };

    (text_len + img_cost + heading_cost + structural_cost).max(40)
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn wrap_text_like_element(
    tag: &tl::HTMLTag,
    _parser: &tl::Parser,
    text: &str,
    keep_id: bool,
) -> String {
    let tag_name = tag.name().as_utf8_str();
    let class = tag
        .attributes()
        .get("class")
        .flatten()
        .map(|v| v.as_utf8_str().to_string());
    let style = tag
        .attributes()
        .get("style")
        .flatten()
        .map(|v| v.as_utf8_str().to_string());
    let id = if keep_id {
        tag.attributes()
            .get("id")
            .flatten()
            .map(|v| v.as_utf8_str().to_string())
    } else {
        None
    };

    let mut attrs = Vec::new();
    if let Some(ref i) = id {
        attrs.push(format!("id=\"{}\"", escape_html(i)));
    }
    if let Some(ref c) = class {
        attrs.push(format!("class=\"{}\"", escape_html(c)));
    }
    if let Some(ref s) = style {
        attrs.push(format!("style=\"{}\"", escape_html(s)));
    }

    let attr_str = if attrs.is_empty() {
        String::new()
    } else {
        format!(" {}", attrs.join(" "))
    };

    format!(
        "<{tag}{attr_str}>{text}</{tag}>",
        tag = tag_name,
        attr_str = attr_str,
        text = escape_html(text)
    )
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paginate_empty() {
        let pages = paginate("", 500);
        assert_eq!(pages.len(), 1);
    }

    #[test]
    fn test_paginate_simple_paragraph() {
        let html = "<p>短短一段话。</p>";
        let pages = paginate(html, 2000);
        assert!(pages.len() >= 1);
        let all: String = pages.join("");
        assert!(all.contains("短短一段话"));
    }

    #[test]
    fn test_split_text_chinese_punctuation() {
        let text = "这是第一句。这是第二句！这是第三句？这是第四句；这是第五句。";
        let chunks = split_text(text, 8);
        assert!(
            chunks.len() > 2,
            "应被标点切分成多段，实际: {}",
            chunks.len()
        );
    }

    #[test]
    fn test_split_text_no_punctuation() {
        let text = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let chunks = split_text(text, 20);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_estimate_block_cost_image() {
        let cost = estimate_block_cost("<img src='a.jpg'>", 10, 1000);
        assert!(cost > 600);
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<p>"), "&lt;p&gt;");
        assert_eq!(escape_html("a & b"), "a &amp; b");
    }

    #[test]
    fn test_paginate_long_chapter() {
        let mut paragraphs = String::new();
        for i in 0..100 {
            paragraphs.push_str(&format!(
                "<p>这是第{}段的测试内容。包含足够的文字来填满视口。</p>\n",
                i
            ));
        }
        let pages = paginate(&paragraphs, 1000);
        assert!(
            pages.len() > 5,
            "长章节应该被切分成多页，实际: {}",
            pages.len()
        );
    }

    #[test]
    fn test_paginate_preserves_image_blocks() {
        let html = "<p>前面文字</p><img src='cover.jpg'><p>后面文字</p>";
        let pages = paginate(html, 500);
        let all: String = pages.join("");
        assert!(all.contains("<img"));
    }
}
