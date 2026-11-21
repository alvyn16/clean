//! Download and cache images embedded in emails so GPUI can render them,
//! including animated GIF/WebP via GPUI's native image decoder.

use std::io::Write;

use anyhow::Result;

use crate::config;

/// Cache path for a remote image URL (hashed so names stay safe).
pub fn cache_path(url: &str) -> std::path::PathBuf {
    use md5::Digest;
    let hash = md5::Md5::digest(url.as_bytes());
    let name: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    config::data_dir().join("email-images").join(format!("{name}.img"))
}

/// Fetch an image into the local cache. Returns the cached path.
pub fn cache_image(url: &str) -> Result<std::path::PathBuf> {
    let path = cache_path(url);
    if path.exists() {
        return Ok(path);
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .connect_timeout(std::time::Duration::from_secs(2))
        .build()?;
    let response = client.get(url).send()?.error_for_status()?;
    let bytes = response.bytes()?;
    // Tiny responses are usually CDN placeholders/error pages, not real images.
    if bytes.len() < 200 {
        return Err(anyhow::anyhow!("image too small (placeholder)"));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(&path)?;
    file.write_all(&bytes)?;
    Ok(path)
}

/// Rewrite remote `<img src>` URLs in HTML to locally cached `file://` paths.
/// Images that aren't cached yet are left as-is (Chromium will try, but with
/// network blocked they fail fast). Returns the rewritten HTML.
pub fn localize_images(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find("src=") {
        result.push_str(&rest[..pos]);
        let after = &rest[pos + 4..];
        // Handle src="..." and src='...'
        let (quote, value_start) = match after.as_bytes().first() {
            Some(b'"') => (b'"', 1),
            Some(b'\'') => (b'\'', 1),
            _ => {
                result.push_str("src=");
                rest = after;
                continue;
            }
        };
        let Some(end) = after[value_start..].find(quote as char) else {
            result.push_str("src=");
            rest = after;
            continue;
        };
        let url = &after[value_start..value_start + end];
        if (url.starts_with("http://") || url.starts_with("https://"))
            && cache_path(url).exists()
        {
            let local = cache_path(url);
            result.push_str("src=\"file://");
            result.push_str(&local.to_string_lossy());
            result.push('"');
        } else {
            result.push_str("src=\"");
            result.push_str(url);
            result.push('"');
        }
        rest = &after[value_start + end + 1..];
    }
    result.push_str(rest);
    result
}

/// Extract all remote image URLs from raw HTML (src attributes and CSS url()).
pub fn extract_image_urls(html: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for marker in ["src=", "url("] {
        let mut rest = html;
        loop {
            let Some(pos) = rest.find(marker) else { break };
            let mut after = &rest[pos + marker.len()..];
            // Skip whitespace and an optional quote after the marker.
            after = after.trim_start_matches([' ', '\t', '\r', '\n', '"', '\'']);
            let end = after
                .find(['"', '\'', ')', ' ', '>', '\r', '\n'])
                .unwrap_or(after.len().min(300));
            let url = after[..end].trim();
            if (url.starts_with("http://") || url.starts_with("https://"))
                && !urls.iter().any(|u: &String| u == url)
            {
                urls.push(url.to_string());
            }
            rest = &after[end.min(after.len())..];
        }
    }
    urls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_image_urls() {
        let html = r#"<img src="https://a.com/1.png"><img src='https://b.com/2.jpg'>
            <div style="background:url(https://c.com/3.gif)"></div>
            <img src="cid:inline"><img src="https://a.com/1.png">"#;
        let urls = extract_image_urls(html);
        assert_eq!(urls.len(), 3, "got {urls:?}");
        assert!(urls.contains(&"https://a.com/1.png".to_string()));
        assert!(urls.contains(&"https://b.com/2.jpg".to_string()));
        assert!(urls.contains(&"https://c.com/3.gif".to_string()));
    }
}

/// Rewrite remote image URLs to inline base64 data URIs from the local cache.
/// Data URIs always load in Chromium, unlike file:// subresources which CDP
/// may block. Returns the rewritten HTML.
pub fn inline_images(html: &str) -> String {
    use base64::Engine;
    let mut result = String::with_capacity(html.len() * 2);
    let mut rest = html;
    while let Some(pos) = rest.find("src=") {
        result.push_str(&rest[..pos]);
        let after = &rest[pos + 4..];
        let after = after.trim_start_matches([' ', '\t', '\r', '\n', '"', '\'']);
        let end = after
            .find(['"', '\'', ' ', '>', '\r', '\n'])
            .unwrap_or(after.len().min(400));
        let url = after[..end].trim();
        if (url.starts_with("http://") || url.starts_with("https://"))
            && cache_path(url).exists()
        {
            if let Ok(bytes) = std::fs::read(cache_path(url)) {
                let mime = if bytes.starts_with(b"\x89PNG") {
                    "image/png"
                } else if bytes.starts_with(b"\xff\xd8") {
                    "image/jpeg"
                } else if bytes.starts_with(b"GIF") {
                    "image/gif"
                } else if bytes.starts_with(b"RIFF") {
                    "image/webp"
                } else {
                    "application/octet-stream"
                };
                let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
                result.push_str("src=\"data:");
                result.push_str(mime);
                result.push_str(";base64,");
                result.push_str(&encoded);
                result.push('"');
            } else {
                result.push_str("src=\"");
                result.push_str(url);
                result.push('"');
            }
        } else {
            result.push_str("src=\"");
            result.push_str(url);
            result.push('"');
        }
        rest = &after[end.min(after.len())..];
    }
    result.push_str(rest);
    result
}

#[test]
fn verify_inline_count() {
    let html = include_str!("fixtures/aliexpress_email.html");
    let inlined = inline_images(html);
    let count = inlined.matches("data:image/").count();
    println!("inlined data URIs: {count}");
    assert!(count >= 10, "expected >=10 inlined images, got {count}");
}
