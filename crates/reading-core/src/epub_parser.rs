use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

use crate::html_sanitizer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpubMetadata {
    pub title: String,
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocItem {
    pub label: String,
    pub href: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub subitems: Vec<TocItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpineItem {
    pub id: String,
    pub href: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookInfo {
    pub metadata: EpubMetadata,
    pub toc: Vec<TocItem>,
    pub spine: Vec<SpineItem>,
}

fn parse_book_info_from_archive<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<BookInfo, String> {
    let opf_path = find_opf_path(archive)?;
    let opf_dir = Path::new(&opf_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let opf_content = read_zip_entry(archive, &opf_path)?;
    let (metadata, manifest, spine) = parse_opf(&opf_content)?;

    let toc = parse_toc(archive, &manifest, &opf_dir, &opf_content, &spine)?;

    Ok(BookInfo {
        metadata,
        toc,
        spine,
    })
}

pub fn parse_book_info(data: &[u8]) -> Result<BookInfo, String> {
    let cursor = Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("无法解析 ZIP/EPUB: {}", e))?;
    parse_book_info_from_archive(&mut archive)
}

fn find_opf_path<R: std::io::Read + std::io::Seek>(archive: &mut zip::ZipArchive<R>) -> Result<String, String> {
    let container = read_zip_entry(
        archive,
        "META-INF/container.xml",
    )?;
    parse_container(&container)
}

fn parse_container(xml: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    reader.config_mut().expand_empty_elements = true;

    let mut buf = Vec::new();
    let mut full_path: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == b"rootfile" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"full-path" {
                            full_path = Some(
                                String::from_utf8_lossy(&attr.value).to_string(),
                            );
                        }
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                if e.name().as_ref() == b"rootfile" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"full-path" {
                            full_path = Some(
                                String::from_utf8_lossy(&attr.value).to_string(),
                            );
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    if let Some(path) = full_path {
        return Ok(path);
    }

    // 简单字符串匹配兜底
    let lower = xml.to_lowercase();
    if let Some(start) = lower.find("full-path") {
        let after = &lower[start + 9..];
        if let Some(quote_start) = after.find('"') {
            let val_start = start + 10 + quote_start;
            if let Some(quote_end) = xml[val_start..].find('"') {
                let val = &xml[val_start..val_start + quote_end];
                return Ok(val.trim().to_string());
            }
        }
    }

    Err("container.xml 中未找到 rootfile".to_string())
}

#[derive(Debug, Clone)]
pub struct ManifestItem {
    pub id: String,
    pub href: String,
    pub media_type: String,
}

fn resolve_path(base_dir: &str, href: &str) -> String {
    if href.is_empty() {
        return String::new();
    }
    if base_dir.is_empty() {
        return href.to_string();
    }
    let mut parts: Vec<&str> = base_dir.split('/').filter(|s| !s.is_empty()).collect();
    for seg in href.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(seg),
        }
    }
    parts.join("/")
}

fn parse_opf(xml: &str) -> Result<(EpubMetadata, HashMap<String, ManifestItem>, Vec<SpineItem>), String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut metadata = EpubMetadata {
        title: String::new(),
        author: None,
    };
    let mut manifest: HashMap<String, ManifestItem> = HashMap::new();
    let mut spine: Vec<SpineItem> = Vec::new();
    let mut guide_cover: Option<String> = None; // 封面 href

    let mut current_tag = String::new();

    // 收集 manifest id → (href, media_type) 映射
    let mut manifest_hrefs: HashMap<String, String> = HashMap::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match current_tag.as_str() {
                    "item" | "opf:item" => {
                        let mut id = String::new();
                        let mut href = String::new();
                        let mut media_type = String::new();
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"id" => id = String::from_utf8_lossy(&attr.value).to_string(),
                                b"href" => href = String::from_utf8_lossy(&attr.value).to_string(),
                                b"media-type" => media_type = String::from_utf8_lossy(&attr.value).to_string(),
                                _ => {}
                            }
                        }
                        if !id.is_empty() && !href.is_empty() {
                            manifest_hrefs.insert(id.clone(), href.clone());
                            manifest.insert(id.clone(), ManifestItem { id, href, media_type });
                        }
                    }
                    "itemref" | "opf:itemref" => {
                        let mut idref = String::new();
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"idref" {
                                idref = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                        if let Some(href) = manifest_hrefs.get(&idref) {
                            spine.push(SpineItem { id: idref, href: href.clone() });
                        }
                    }
                    "reference" => {
                        let mut ref_type = String::new();
                        let mut ref_href = String::new();
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"type" => ref_type = String::from_utf8_lossy(&attr.value).to_string(),
                                b"href" => ref_href = String::from_utf8_lossy(&attr.value).to_string(),
                                _ => {}
                            }
                        }
                        if ref_type == "cover" && !ref_href.is_empty() {
                            guide_cover = Some(ref_href);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "item" | "opf:item" => {
                        let mut id = String::new();
                        let mut href = String::new();
                        let mut media_type = String::new();
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"id" => id = String::from_utf8_lossy(&attr.value).to_string(),
                                b"href" => href = String::from_utf8_lossy(&attr.value).to_string(),
                                b"media-type" => media_type = String::from_utf8_lossy(&attr.value).to_string(),
                                _ => {}
                            }
                        }
                        if !id.is_empty() && !href.is_empty() {
                            manifest_hrefs.insert(id.clone(), href.clone());
                            manifest.insert(id.clone(), ManifestItem { id, href, media_type });
                        }
                    }
                    "itemref" | "opf:itemref" => {
                        let mut idref = String::new();
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"idref" {
                                idref = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                        if let Some(href) = manifest_hrefs.get(&idref) {
                            spine.push(SpineItem { id: idref, href: href.clone() });
                        }
                    }
                    "reference" => {
                        let mut ref_type = String::new();
                        let mut ref_href = String::new();
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"type" => ref_type = String::from_utf8_lossy(&attr.value).to_string(),
                                b"href" => ref_href = String::from_utf8_lossy(&attr.value).to_string(),
                                _ => {}
                            }
                        }
                        if ref_type == "cover" && !ref_href.is_empty() {
                            guide_cover = Some(ref_href);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                let t = e.unescape().unwrap_or_default().to_string();
                match current_tag.as_str() {
                    "title" | "dc:title" => { metadata.title = format!("{}{}", metadata.title, t); }
                    "creator" | "dc:creator" => {
                        metadata.author = Some(format!("{}{}", metadata.author.as_deref().unwrap_or(""), t));
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let tag_bytes = e.name().0.to_vec();
                let tag = String::from_utf8_lossy(&tag_bytes);
                if tag == "title" || tag == "dc:title" || tag == "creator" || tag == "dc:creator" {
                    current_tag.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    // guide 封面：若不在 spine 中则插入首部
    if let Some(ref href) = guide_cover {
        let already_in_spine = spine.iter().any(|s| s.href == *href || s.href.ends_with(href));
        if !already_in_spine {
            spine.insert(0, SpineItem {
                id: "cover".into(),
                href: href.clone(),
            });
        }
    }

    Ok((metadata, manifest, spine))
}

fn parse_toc<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    manifest: &HashMap<String, ManifestItem>,
    opf_dir: &str,
    opf_content: &str,
    spine: &[SpineItem],
) -> Result<Vec<TocItem>, String> {
    // 先尝试从 OPF spine toc 属性找到 NCX 文件
    let ncx_href = find_ncx_href(opf_content, manifest);

    if let Some(href) = ncx_href {
        let ncx_path = resolve_path(opf_dir, &href);
        let ncx_dir = Path::new(&ncx_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if let Ok(content) = read_zip_entry(archive, &ncx_path) {
            let mut toc = parse_ncx(&content, &ncx_dir)?;
            normalize_toc_labels(&mut toc);
            return Ok(toc);
        }
    }

    // 尝试查找 nav.xhtml (EPUB 3)
    for item in manifest.values() {
        if item.media_type == "application/xhtml+xml"
            && (item.href.contains("nav") || item.id.contains("nav"))
        {
            let nav_path = resolve_path(opf_dir, &item.href);
            let nav_dir = Path::new(&nav_path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            if let Ok(content) = read_zip_entry(archive, &nav_path) {
                if let Ok(mut toc) = parse_nav_xhtml(&content, &nav_dir) {
                    if !toc.is_empty() {
                        normalize_toc_labels(&mut toc);
                        return Ok(toc);
                    }
                }
            }
        }
    }

    // 兜底：用 spine 构建简单 TOC
    let mut toc = Vec::new();
    for (i, item) in spine.iter().enumerate() {
        let label = std::path::Path::new(&item.href)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("第 {} 章", i + 1));
        toc.push(TocItem {
            label,
            href: item.href.clone(),
            subitems: vec![],
        });
    }
    Ok(toc)
}

fn normalize_toc_labels(items: &mut [TocItem]) {
    for (idx, item) in items.iter_mut().enumerate() {
        normalize_toc_labels(&mut item.subitems);

        let trimmed = item.label.trim();
        if is_internal_filename_label(trimmed, &item.href) {
            if item.subitems.is_empty() {
                item.label = format!("第{}章", idx + 1);
            } else {
                // Pure grouping nodes in some NCX files use filenames like Volume01.xhtml.
                // Hide those labels; the frontend will render their children directly.
                item.label.clear();
                item.href.clear();
            }
        } else if trimmed != item.label {
            item.label = trimmed.to_string();
        }
    }
}

fn is_internal_filename_label(label: &str, href: &str) -> bool {
    let lower = label.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return true;
    }
    if lower.ends_with(".xhtml")
        || lower.ends_with(".html")
        || lower.ends_with(".htm")
        || lower.contains(".xhtml.")
        || lower.contains(".html.")
    {
        return true;
    }

    let href_name = href
        .split('#')
        .next()
        .unwrap_or(href)
        .rsplit('/')
        .next()
        .unwrap_or(href)
        .to_ascii_lowercase();
    let href_stem = href_name
        .strip_suffix(".xhtml")
        .or_else(|| href_name.strip_suffix(".html"))
        .or_else(|| href_name.strip_suffix(".htm"))
        .unwrap_or(&href_name);

    lower == href_name || lower == href_stem
}

fn find_ncx_href(opf_xml: &str, manifest: &HashMap<String, ManifestItem>) -> Option<String> {
    let mut reader = Reader::from_str(opf_xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == b"spine" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"toc" {
                            let ncx_id =
                                String::from_utf8_lossy(&attr.value).to_string();
                            if let Some(item) = manifest.get(&ncx_id) {
                                return Some(item.href.clone());
                            }
                        }
                    }
                    // 部分 EPUB 不声明 toc，尝试从 manifest 找 ncx
                    for item in manifest.values() {
                        if item.media_type == "application/x-dtbncx+xml" {
                            return Some(item.href.clone());
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    // 扫描 manifest 找 NCX
    for item in manifest.values() {
        if item.media_type == "application/x-dtbncx+xml" {
            return Some(item.href.clone());
        }
    }

    None
}

fn parse_ncx(xml: &str, opf_dir: &str) -> Result<Vec<TocItem>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    // NCX 的 <content src=".."/> 是自闭合元素；展开成 Start+End，
    // 否则它走 Event::Empty 被忽略，导致所有目录项 href 为空、点不了。
    reader.config_mut().expand_empty_elements = true;

    let mut buf = Vec::new();
    let mut toc = Vec::new();
    let mut stack: Vec<TocItem> = Vec::new();
    let mut label_buf = String::new();
    let mut in_label = false;
    let mut pending_href: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "navPoint" => {
                        stack.push(TocItem {
                            label: String::new(),
                            href: String::new(),
                            subitems: vec![],
                        });
                    }
                    "content" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"src" {
                                pending_href = Some(
                                    String::from_utf8_lossy(&attr.value).to_string(),
                                );
                            }
                        }
                    }
                    "navLabel" | "text" if !in_label => {
                        in_label = true;
                        label_buf.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) if in_label => {
                label_buf.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "text" | "navLabel" => {
                        in_label = false;
                    }
                    "content" => {
                        if let (Some(href), Some(item)) =
                            (pending_href.take(), stack.last_mut())
                        {
                            item.href = resolve_path(opf_dir, &href);
                        }
                    }
                    "navPoint" => {
                        if let Some(item) = stack.pop() {
                            let label = label_buf.trim().to_string();
                            let filled = TocItem {
                                label: if label.is_empty() {
                                    item.href
                                        .rsplit('/')
                                        .next()
                                        .unwrap_or("")
                                        .to_string()
                                } else {
                                    label
                                },
                                href: item.href,
                                subitems: item.subitems,
                            };
                            label_buf.clear();
                            if let Some(parent) = stack.last_mut() {
                                parent.subitems.push(filled);
                            } else {
                                toc.push(filled);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("NCX 解析失败: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(toc)
}

fn parse_nav_xhtml(xml: &str, opf_dir: &str) -> Result<Vec<TocItem>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    reader.config_mut().expand_empty_elements = true; // 与 NCX 一致，稳妥处理自闭合元素

    let mut buf = Vec::new();
    let mut toc = Vec::new();
    let mut in_toc_nav = false;
    let mut stack: Vec<TocItem> = Vec::new();
    let mut pending_href: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();

                // 检测 toc nav 标签
                if tag == "nav" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"epub:type"
                            && attr.value.as_ref() == b"toc"
                        {
                            in_toc_nav = true;
                        }
                    }
                }

                if in_toc_nav {
                    match tag.as_str() {
                        "ol" => {},
                        "li" => {
                            stack.push(TocItem {
                                label: String::new(),
                                href: String::new(),
                                subitems: vec![],
                            });
                        }
                        "a" => {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"href" {
                                    pending_href = Some(
                                        String::from_utf8_lossy(&attr.value).to_string(),
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_toc_nav && pending_href.is_some() {
                    if let Some(item) = stack.last_mut() {
                        item.label
                            .push_str(&e.unescape().unwrap_or_default());
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if tag == "nav" {
                    in_toc_nav = false;
                }

                if in_toc_nav {
                    match tag.as_str() {
                        "ol" => {},
                        "a" => {
                            if let (Some(href), Some(item)) =
                                (pending_href.take(), stack.last_mut())
                            {
                                item.href = resolve_path(opf_dir, &href);
                            }
                        }
                        "li" => {
                            if let Some(item) = stack.pop() {
                                if let Some(parent) = stack.last_mut() {
                                    parent.subitems.push(item);
                                } else {
                                    toc.push(item);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(toc)
}

fn read_zip_entry<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<String, String> {
    let name_normalized = name.to_lowercase().replace('\\', "/");
    let name_basename = name_normalized.rsplit('/').next().unwrap_or(&name_normalized);

    // 先精确查找
    let mut found_name: Option<String> = None;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let entry_lower = entry.name().to_lowercase().replace('\\', "/");
        if entry_lower == name_normalized {
            found_name = Some(entry.name().to_string());
            break;
        }
    }

    // 如果精确找不到，用文件名（最后一段）查找
    if found_name.is_none() {
        for i in 0..archive.len() {
            let entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let entry_lower = entry.name().to_lowercase().replace('\\', "/");
            let entry_basename = entry_lower.rsplit('/').next().unwrap_or(&entry_lower);
            if entry_basename == name_basename {
                found_name = Some(entry.name().to_string());
                break;
            }
        }
    }

    let entry_name = found_name
        .ok_or_else(|| format!("ZIP 中未找到: {} ({} 条目)", name, archive.len()))?;
    let mut entry = archive
        .by_name(&entry_name)
        .map_err(|e| format!("读取 ZIP 条目失败 '{}': {}", entry_name, e))?;

    let raw = read_entry_bytes(&mut entry)?;
    decode_text(&raw)
}

/// 从 EPUB 字节里按路径读取图片，返回 (mime, 字节)。精确匹配 → 后缀 → 文件名兜底。
pub fn read_image_from_zip(data: &[u8], path: &str) -> Option<(String, Vec<u8>)> {
    let cursor = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor).ok()?;

    let want = path.replace('\\', "/");
    let want = want.trim_start_matches("./").replace("../", "");
    let want_lower = want.to_lowercase();
    let want_base = want_lower.rsplit('/').next().unwrap_or(&want_lower).to_string();

    let mut found: Option<String> = None;
    for i in 0..archive.len() {
        let e = archive.by_index(i).ok()?;
        let nl = e.name().to_lowercase().replace('\\', "/");
        if nl == want_lower || nl.ends_with(&format!("/{}", want_lower)) {
            found = Some(e.name().to_string());
            break;
        }
    }
    if found.is_none() {
        for i in 0..archive.len() {
            let e = archive.by_index(i).ok()?;
            let nl = e.name().to_lowercase().replace('\\', "/");
            let nb = nl.rsplit('/').next().unwrap_or(&nl);
            if nb == want_base {
                found = Some(e.name().to_string());
                break;
            }
        }
    }
    let name = found?;
    let mime = mime_from_ext(&name);
    let mut entry = archive.by_name(&name).ok()?;
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut entry, &mut buf).ok()?;
    Some((mime, buf))
}

fn mime_from_ext(name: &str) -> String {
    let n = name.to_lowercase();
    let m = if n.ends_with(".png") {
        "image/png"
    } else if n.ends_with(".jpg") || n.ends_with(".jpeg") {
        "image/jpeg"
    } else if n.ends_with(".gif") {
        "image/gif"
    } else if n.ends_with(".svg") {
        "image/svg+xml"
    } else if n.ends_with(".webp") {
        "image/webp"
    } else {
        "application/octet-stream"
    };
    m.to_string()
}

fn read_entry_bytes(entry: &mut zip::read::ZipFile) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    std::io::Read::read_to_end(entry, &mut buf)
        .map_err(|e| format!("读取条目失败: {}", e))?;
    Ok(buf)
}

fn decode_text(raw: &[u8]) -> Result<String, String> {
    let (bom_encoding, bom_len) = encoding_rs::Encoding::for_bom(raw)
        .unwrap_or((encoding_rs::UTF_8, 0));

    let raw_no_bom = &raw[bom_len..];

    if bom_len > 0 && raw.starts_with(b"\xfe\xff") {
        return decode_utf16(raw, true);
    }
    if bom_len > 0 && raw.starts_with(b"\xff\xfe") {
        return decode_utf16(raw, false);
    }

    if bom_len > 0 {
        let (cow, _, _) = bom_encoding.decode(raw_no_bom);
        return Ok(cow.into_owned());
    }

    let declared_enc = detect_encoding_from_content(raw_no_bom);

    if let Some(enc) = declared_enc {
        let (cow, _, _) = enc.decode(raw_no_bom);
        return Ok(cow.into_owned());
    }

    let (utf8_result, _, had_errors) = encoding_rs::UTF_8.decode(raw_no_bom);
    if !had_errors {
        return Ok(utf8_result.into_owned());
    }

    let gbk = encoding_rs::Encoding::for_label("gbk".as_bytes()).unwrap_or(encoding_rs::UTF_8);
    let (gbk_result, _, _) = gbk.decode(raw_no_bom);
    Ok(gbk_result.into_owned())
}

fn detect_encoding_from_content(raw: &[u8]) -> Option<&'static encoding_rs::Encoding> {
    let head = String::from_utf8_lossy(&raw[..raw.len().min(512)]).to_lowercase();

    // XML encoding 声明
    if let Some(enc_start) = head.find("encoding=") {
        let after = &head[enc_start + 9..];
        let quote = after.chars().next().unwrap_or('"');
        if let Some(enc_end) = after[1..].find(quote) {
            let enc_name = &after[1..enc_end + 1];
            if let Some(enc) = encoding_rs::Encoding::for_label(enc_name.as_bytes()) {
                return Some(enc);
            }
        }
    }

    // HTML5 charset
    if let Some(charset_pos) = head.find("charset=") {
        let after = &head[charset_pos + 8..];
        let enc_name = after
            .split(|c: char| c == '"' || c == '\'' || c == '>' || c == '/' || c == ' ')
            .next()
            .unwrap_or("");
        if !enc_name.is_empty() {
            if let Some(enc) = encoding_rs::Encoding::for_label(enc_name.as_bytes()) {
                return Some(enc);
            }
        }
    }

    None
}

fn decode_utf16(raw: &[u8], big_endian: bool) -> Result<String, String> {
    let raw = if raw.len() < 2 { raw } else { &raw[2..] };
    let u16s: Vec<u16> = if big_endian {
        raw.chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect()
    } else {
        raw.chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect()
    };
    String::from_utf16(&u16s).map_err(|e| format!("UTF-16 解码失败: {}", e))
}

/// 按需解析单个章节（缓存未命中时调用）
pub fn parse_single_chapter(
    data: &[u8],
    href: &str,
    book_info: &BookInfo,
) -> Result<String, String> {
    if href.trim().is_empty() {
        return Err("空章节 href".to_string());
    }
    let cursor = Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("无法打开 ZIP: {}", e))?;

    let opf_path = find_opf_path(&mut archive)?;
    let opf_dir = Path::new(&opf_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let spine_item = book_info
        .spine
        .iter()
        .find(|s| s.href == href)
        .or_else(|| {
            let basename = href.rsplit('/').next().unwrap_or(href);
            book_info.spine.iter().find(|s| {
                s.href.rsplit('/').next().unwrap_or(&s.href) == basename
            })
        })
        .ok_or_else(|| format!("spine 中未找到章节: {}", href))?;

    let resolved = resolve_path(&opf_dir, &spine_item.href);
    let raw = read_zip_entry(&mut archive, &resolved)
        .or_else(|_| read_zip_entry(&mut archive, &spine_item.href))?;

    let opf_content = read_zip_entry(&mut archive, &opf_path)?;
    let (_, manifest, _) = parse_opf(&opf_content)?;

    html_sanitizer::clean_chapter(&raw, &opf_dir, &manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 复现并锁定"目录点不了"的回归：NCX 自闭合 <content src/> 必须解析出 href。
    #[test]
    fn ncx_self_closing_content_populates_href() {
        let ncx = r#"<?xml version="1.0" encoding="utf-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/"><navMap>
  <navPoint><navLabel><text>第一章</text></navLabel><content src="Text/ch1.html"/></navPoint>
  <navPoint><navLabel><text>第二章</text></navLabel><content src="Text/ch2.html"/>
    <navPoint><navLabel><text>1.1 节</text></navLabel><content src="Text/ch2.html#s1"/></navPoint>
  </navPoint>
</navMap></ncx>"#;

        let toc = parse_ncx(ncx, "OEBPS").unwrap();
        assert_eq!(toc.len(), 2, "顶层应有两章");
        assert_eq!(toc[0].label, "第一章");
        assert_eq!(toc[0].href, "OEBPS/Text/ch1.html", "href 不能为空（修复前正是空）");
        assert_eq!(toc[1].subitems.len(), 1, "第二章应有一个子节点");
        assert_eq!(toc[1].subitems[0].href, "OEBPS/Text/ch2.html#s1");
    }

    #[test]
    fn toc_filename_labels_are_hidden_for_group_nodes() {
        let mut toc = vec![TocItem {
            label: "Volume01.xhtml.xhtml".to_string(),
            href: "OEBPS/Text/Volume01.xhtml".to_string(),
            subitems: vec![TocItem {
                label: "第二版出版说明".to_string(),
                href: "OEBPS/Text/Section0001.xhtml".to_string(),
                subitems: vec![],
            }],
        }];

        normalize_toc_labels(&mut toc);

        assert_eq!(toc[0].label, "");
        assert_eq!(toc[0].href, "");
        assert_eq!(toc[0].subitems[0].label, "第二版出版说明");
    }
}
