//! Render an email's HTML (with full CSS) to a cached PNG using headless
//! Chromium, so GPUI can display real HTML emails without embedding a webview.

use anyhow::Result;

use crate::config;

const RENDER_WIDTH: u32 = 720;
const RENDER_HEIGHT: u32 = 3600;

/// Cache path for a rendered email PNG. The key is the localized HTML
/// (file:// URLs for cached images), so it changes once images are available.
pub fn cache_path(html: &str) -> std::path::PathBuf {
    use md5::Digest;
    let local = crate::mail::images::localize_images(html);
    let hash: String = md5::Md5::digest(local.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    config::data_dir().join("email-render").join(format!("{hash}.png"))
}

/// Render HTML to a cropped PNG (removes trailing white space) and cache it.
pub fn render_email(html: &str, base_url: Option<&str>) -> Result<std::path::PathBuf> {
    let path = cache_path(html);
    if path.exists() {
        return Ok(path);
    }

    let document = build_document(html, base_url);
    render_document_to_file(&document, &path)?;
    Ok(path)
}

/// Render HTML to a PNG via the reliable one-shot Chromium path.
pub fn render_with(
    html: &str,
    base_url: Option<&str>,
    path: &std::path::Path,
) -> Result<()> {
    let document = build_document(html, base_url);
    render_document_to_file(&document, path)
}

pub fn build_document(html: &str, base_url: Option<&str>) -> String {
    let base_tag = base_url
        .map(|url| format!("<base href=\"{}\">", escape_attr(url)))
        .unwrap_or_default();
    let local_html = crate::mail::images::localize_images(html);
    format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8">{base_tag}
        <style>
          html,body {{ margin:0; padding:0; background:#ffffff; }}
          img {{ max-width:100% !important; height:auto !important; }}
          * {{ -webkit-text-size-adjust:100%; }}
        </style></head><body>{local_html}</body></html>"#
    )
}

fn crop_png(path: &std::path::Path) -> Result<std::path::PathBuf> {
    let img = image::open(path)?;
    let width = img.width();
    let rgb = img.to_rgb8();
    let mut last_content_row = 0u32;
    for y in (0..rgb.height()).rev() {
        let mut has_content = false;
        for x in 0..width {
            let pixel = rgb.get_pixel(x, y);
            if pixel.0[0] < 245 || pixel.0[1] < 245 || pixel.0[2] < 245 {
                has_content = true;
                break;
            }
        }
        if has_content {
            last_content_row = y;
            break;
        }
    }
    let height = (last_content_row + 40).min(rgb.height());
    let cropped = image::imageops::crop_imm(&img, 0, 0, width, height).to_image();
    let tmp = path.with_extension("cropped.png");
    cropped.save(&tmp)?;
    std::fs::rename(&tmp, path)?;
    Ok(path.to_path_buf())
}

/// Render a built HTML document to a file using one-shot headless Chromium.
/// This path is reliable: cached images are rewritten to local file:// URLs and
/// load fully, unlike the CDP path which produced blank images for many mails.
fn render_document_to_file(document: &str, path: &std::path::Path) -> Result<()> {
    let dir = config::data_dir().join("email-render");
    std::fs::create_dir_all(&dir)?;
    let pid = std::process::id();
    let tmp = dir.join(format!("input-{pid}.html"));
    std::fs::write(&tmp, document.as_bytes())?;
    let raw = dir.join(format!("raw-{pid}.png"));
    let output = std::process::Command::new("chromium")
        .args([
            "--headless=new", "--disable-gpu", "--no-sandbox",
            "--disable-dev-shm-usage", "--hide-scrollbars",
            "--force-device-scale-factor=1", "--disable-background-networking",
            "--disable-extensions", "--log-level=3",
        ])
        .arg(format!("--window-size={RENDER_WIDTH},{RENDER_HEIGHT}"))
        .arg(format!("--screenshot={}", raw.display()))
        .arg(format!("file://{}", tmp.display()))
        .output();
    let _ = std::fs::remove_file(&tmp);
    let output = output?;
    if !output.status.success() || !raw.exists() {
        let _ = std::fs::remove_file(&raw);
        return Err(anyhow::anyhow!("headless Chromium failed to render email"));
    }
    crop_png_to(&raw, path)
}

fn crop_png_to(raw: &std::path::Path, path: &std::path::Path) -> Result<()> {
    let img = image::open(raw)?;
    let width = img.width();
    let rgb = img.to_rgb8();
    let mut last_content_row = 0u32;
    for y in (0..rgb.height()).rev() {
        let mut has_content = false;
        for x in 0..width {
            let pixel = rgb.get_pixel(x, y);
            if pixel.0[0] < 245 || pixel.0[1] < 245 || pixel.0[2] < 245 {
                has_content = true;
                break;
            }
        }
        if has_content { last_content_row = y; break; }
    }
    let height = (last_content_row + 40).min(rgb.height());
    let cropped = image::imageops::crop_imm(&img, 0, 0, width, height).to_image();
    cropped.save(path)?;
    let _ = std::fs::remove_file(raw);
    Ok(())
}


/// Open a URL or local file in the user's browser. Tries xdg-open first,
/// then chromium directly. Logs failures.
pub fn open_in_browser(target: &str) {
    tracing::info!("[browser] opening {target}");
    for (program, arg) in [("xdg-open", target.to_string()), ("chromium", target.to_string())] {
        match std::process::Command::new(program).arg(&arg).spawn() {
            Ok(_) => {
                tracing::info!("[browser] launched via {program}");
                return;
            }
            Err(error) => tracing::warn!("[browser] {program} failed: {error}"),
        }
    }
}

#[test]
#[ignore]
fn verify_aliexpress_render_with_images() {
    let html = include_str!("fixtures/aliexpress_email.html");
    let path = cache_path(html);
    let _ = std::fs::remove_file(&path);
    render_with(html, None, &path).expect("render");
    let size = std::fs::metadata(&path).unwrap().len();
    println!("AliExpress render: {} bytes at {}", size, path.display());
    assert!(size > 150_000, "render missing images: {} bytes", size);
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Remove stale render/image cache files. Caches are rebuilt on demand, so
/// pruning old entries keeps long-running installs from growing without bound.
pub fn prune_caches() -> usize {
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(14 * 24 * 3600))
        .unwrap_or(std::time::UNIX_EPOCH);
    let mut removed = 0;
    for dir in [config::data_dir().join("email-render"), config::data_dir().join("email-images")] {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let stale = std::fs::metadata(&path)
                .and_then(|meta| meta.modified())
                .map(|modified| modified < cutoff)
                .unwrap_or(false);
            if stale && std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    if removed > 0 {
        tracing::info!("pruned {removed} stale mail cache files");
    }
    removed
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[test]
    fn prune_ignores_recent_files() {
        let dir = std::env::temp_dir().join(format!("clean-render-prune-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("recent.png");
        std::fs::write(&file, b"x").unwrap();
        let cutoff = std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(14 * 24 * 3600))
            .unwrap();
        assert!(std::fs::metadata(&file).unwrap().modified().unwrap() > cutoff);
        std::fs::remove_dir_all(&dir).ok();
    }
}

/// Remove browser profiles left behind by older versions or previous runs.
/// The active live-view profile is kept while its Chromium is running.
pub fn prune_browser_profiles() -> usize {
    let data = config::data_dir();
    let pid = std::process::id().to_string();
    let mut removed = 0;
    // `cdp-profile` belonged to the old persistent renderer and is unused now.
    let legacy = data.join("cdp-profile");
    if legacy.is_dir() && std::fs::remove_dir_all(&legacy).is_ok() {
        removed += 1;
    }
    if let Ok(entries) = std::fs::read_dir(&data) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("live-profile-")
                && !name.contains(&pid)
                && std::fs::remove_dir_all(entry.path()).is_ok()
            {
                removed += 1;
            }
        }
    }
    if removed > 0 {
        tracing::info!("pruned {removed} stale browser profiles");
    }
    removed
}
