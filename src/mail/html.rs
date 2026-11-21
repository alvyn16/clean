//! Lightweight HTML-email → block parser.
//!
//! Produces a small, sanitized block list that the GPUI reader can render
//! with plain `div`/text/img elements. Scripts, styles, hidden preheaders,
//! tracking pixels, and long tracking URLs are removed.

use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

#[derive(Clone, Debug, Default)]
pub struct EmailBlock {
    pub kind: BlockKind,
    pub text: String,
    pub href: Option<String>,
    pub src: Option<String>,
    pub alt: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlockKind {
    #[default]
    Paragraph,
    Heading,
    Quote,
    Divider,
    Link,
    Image,
    Video,
}

const HIDDEN_HINTS: [&str; 4] = [
    "display:none",
    "display: none",
    "visibility:hidden",
    "visibility: hidden",
];

/// Parse an HTML email into sanitized blocks.
pub fn parse_email_html(html: &str) -> Vec<EmailBlock> {
    let dom = parse_document(RcDom::default(), Default::default()).one(html.to_string());
    let mut blocks = Vec::new();
    walk(dom.document, &mut blocks, false);
    blocks
}

fn walk(handle: Handle, out: &mut Vec<EmailBlock>, hidden: bool) {
    let node = handle;
    let is_hidden = hidden || node_is_hidden(&node);
    match &node.data {
        NodeData::Text { contents } => {
            if !is_hidden {
                let text = collapse_ws(&contents.borrow());
                if !text.is_empty() {
                    push_text(out, &text);
                }
            }
        }
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.as_ref();
            let attrs = attrs.borrow();
            let attr = |key: &str| -> Option<String> {
                attrs
                    .iter()
                    .find(|a| a.name.local.as_ref() == key)
                    .map(|a| a.value.trim().to_string())
            };

            match tag {
                "script" | "style" | "head" | "noscript" => return,
                "br" => {
                    if !is_hidden {
                        push_text(out, "\n");
                    }
                }
                "hr" => {
                    if !is_hidden {
                        out.push(EmailBlock {
                            kind: BlockKind::Divider,
                            ..Default::default()
                        });
                    }
                }
                "img" => {
                    if !is_hidden
                        && let Some(src) = attr("src") {
                            if is_tracking_pixel(&src, &attr("width"), &attr("height")) {
                                return;
                            }
                            if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
                                out.push(EmailBlock {
                                    kind: BlockKind::Image,
                                    src: Some(src),
                                    alt: attr("alt"),
                                    ..Default::default()
                                });
                            }
                        }
                }
                "video" => {
                    if !is_hidden {
                        let src = attr("src").or_else(|| {
                            node.children.borrow().iter().find_map(|c| match &c.data {
                                NodeData::Element { name, attrs, .. }
                                    if name.local.as_ref() == "source" =>
                                {
                                    attrs
                                        .borrow()
                                        .iter()
                                        .find(|a| a.name.local.as_ref() == "src")
                                        .map(|a| a.value.to_string())
                                }
                                _ => None,
                            })
                        });
                        if let Some(src) = src {
                            out.push(EmailBlock {
                                kind: BlockKind::Video,
                                text: attr("title").unwrap_or_default(),
                                src: Some(src),
                                ..Default::default()
                            });
                        }
                    }
                }
                "h1" | "h2" | "h3" | "h4" => {
                    let mut buf = String::new();
                    collect_text(&node, &mut buf, is_hidden);
                    if !buf.trim().is_empty() {
                        out.push(EmailBlock {
                            kind: BlockKind::Heading,
                            text: buf.trim().to_string(),
                            ..Default::default()
                        });
                    }
                }
                "a" => {
                    let mut buf = String::new();
                    collect_text(&node, &mut buf, is_hidden);
                    let label = buf.trim();
                    if !label.is_empty() && !is_tracking_url(label)
                        && let Some(href) = attr("href")
                            && (href.starts_with("http://")
                                || href.starts_with("https://")
                                || href.starts_with("mailto:"))
                            {
                                out.push(EmailBlock {
                                    kind: BlockKind::Link,
                                    text: label.to_string(),
                                    href: Some(href),
                                    ..Default::default()
                                });
                            }
                }
                _ => {}
            }
        }
        _ => {}
    }

    for child in node.children.borrow().iter() {
        walk(child.clone(), out, is_hidden);
    }
}

fn node_is_hidden(node: &markup5ever_rcdom::Node) -> bool {
    if let NodeData::Element { attrs, .. } = &node.data {
        let attrs = attrs.borrow();
        if let Some(style) = attrs
            .iter()
            .find(|a| a.name.local.as_ref() == "style")
        {
            let value = style.value.to_ascii_lowercase();
            if HIDDEN_HINTS.iter().any(|hint| value.contains(hint)) {
                return true;
            }
        }
        if attrs.iter().any(|a| a.name.local.as_ref() == "hidden") {
            return true;
        }
    }
    false
}

fn collect_text(handle: &Handle, out: &mut String, hidden: bool) {
    let node = handle;
    let is_hidden = hidden || node_is_hidden(node);
    match &node.data {
        NodeData::Text { contents } if !is_hidden => {
            out.push_str(&collapse_ws(&contents.borrow()));
        }
        NodeData::Element { name, .. } if matches!(name.local.as_ref(), "script" | "style") => {}
        _ => {}
    }
    for child in node.children.borrow().iter() {
        collect_text(child, out, is_hidden);
    }
}

fn push_text(out: &mut Vec<EmailBlock>, text: &str) {
    let clean = clean_text(text);
    if clean.is_empty() {
        return;
    }
    // Merge consecutive plain text into one paragraph until a blank line.
    if let Some(last) = out.last_mut()
        && last.kind == BlockKind::Paragraph && !clean.starts_with('\n') {
            if last.text.ends_with('\n') {
                last.text.push_str(&clean);
            } else {
                last.text.push(' ');
                last.text.push_str(&clean);
            }
            return;
        }
    out.push(EmailBlock {
        kind: BlockKind::Paragraph,
        text: clean,
        ..Default::default()
    });
}

fn clean_text(text: &str) -> String {
    let mut result = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !result.is_empty() && !result.ends_with("\n\n") {
                result.push_str("\n\n");
            }
            continue;
        }
        if is_tracking_url(trimmed) || trimmed.starts_with("[[") {
            continue;
        }
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(trimmed);
    }
    result.trim().to_string()
}

fn is_tracking_url(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    (text.starts_with("http://") || text.starts_with("https://"))
        && (lower.contains("/f/a/")
            || lower.contains("tracking")
            || lower.contains("clks")
            || lower.contains("unsubscribe"))
}

fn is_tracking_pixel(src: &str, width: &Option<String>, height: &Option<String>) -> bool {
    if src.contains("track") || src.contains("pixel") || src.contains("open") {
        return true;
    }
    let tiny = |v: &Option<String>| {
        v.as_deref()
            .and_then(|s| s.trim_end_matches("px").trim().parse::<u32>().ok())
            .is_some_and(|n| n <= 2)
    };
    tiny(width) || tiny(height)
}

fn collapse_ws(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut ws = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !ws {
                out.push(' ');
                ws = true;
            }
        } else {
            out.push(ch);
            ws = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_hidden_tracking_and_extracts_content() {
        let html = r#"
        <html><head><style>.x{color:red}</style></head><body>
        <div style="display:none !important">preheader hidden text</div>
        <h1>Big Sale</h1>
        <p>Hello <b>friend</b>, shop now.</p>
        <a href="https://tracking-us.uniqlo.com/f/a/ABC~~/x">[[na0women]]</a>
        <a href="https://shop.uniqlo.com/women">Women</a>
        <img src="https://img.uniqlo.com/hero.jpg" alt="Hero">
        <img src="https://track.open.com/pixel.gif" width="1" height="1">
        <hr>
        <video src="https://video.example.com/clip.mp4" title="Watch"></video>
        </body></html>
        "#;
        let blocks = parse_email_html(html);
        let texts: Vec<&str> = blocks.iter().map(|b| b.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("Big Sale")), "heading missing");
        assert!(texts.iter().any(|t| t.contains("shop now")), "paragraph missing");
        assert!(!texts.iter().any(|t| t.contains("preheader")), "hidden text leaked");
        assert!(!texts.iter().any(|t| t.contains("tracking-us")), "tracking url leaked");
        assert!(blocks.iter().any(|b| b.kind == BlockKind::Link && b.text == "Women"), "clean link missing");
        assert!(blocks.iter().any(|b| b.kind == BlockKind::Image && b.src.as_deref() == Some("https://img.uniqlo.com/hero.jpg")), "image missing");
        assert!(blocks.iter().any(|b| b.kind == BlockKind::Video), "video missing");
        assert!(blocks.iter().any(|b| b.kind == BlockKind::Divider), "divider missing");
        let images: Vec<&EmailBlock> = blocks.iter().filter(|b| b.kind == BlockKind::Image).collect();
        assert_eq!(images.len(), 1, "tracking pixel should be dropped");
    }
}
