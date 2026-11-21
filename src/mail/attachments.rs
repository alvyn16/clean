//! Attachment storage: decoded mail parts live in the app data directory,
//! with metadata indexed in SQLite. Composing keeps the original file paths.

use anyhow::{Context, Result};

use crate::config;
use crate::db::{AttachmentRow, DraftAttachmentRow};

/// Make a filename safe to use on disk without losing its extension.
pub fn sanitize_filename(name: &str) -> String {
    let trimmed = name.trim().replace(['\\', '/'], "_");
    let cleaned: String = trimmed
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\0' => '_',
            _ => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() {
        "attachment.bin".into()
    } else {
        cleaned
    }
}

fn message_dir(message_id: i64) -> std::path::PathBuf {
    config::data_dir().join("attachments").join(message_id.to_string())
}

/// Write decoded attachment bytes to the cache and return DB rows.
pub fn save_message_attachments(
    message_id: i64,
    attachments: &[crate::mail::Attachment],
) -> Result<Vec<AttachmentRow>> {
    if attachments.is_empty() {
        return Ok(Vec::new());
    }
    let dir = message_dir(message_id);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let mut rows = Vec::new();
    for (index, item) in attachments.iter().enumerate() {
        let filename = sanitize_filename(&item.filename);
        let path = dir.join(format!("{index:02}-{filename}"));
        std::fs::write(&path, &item.data)
            .with_context(|| format!("writing attachment {}", path.display()))?;
        rows.push(AttachmentRow {
            id: 0,
            message_id,
            filename: item.filename.clone(),
            mime: item.mime.clone(),
            size: item.data.len(),
            path: path.to_string_lossy().into_owned(),
        });
    }
    Ok(rows)
}

/// Cache files parsed out of a draft fetched from the IMAP Drafts folder.
pub fn save_draft_attachments(
    draft_id: i64,
    attachments: &[crate::mail::Attachment],
) -> Result<Vec<DraftAttachmentRow>> {
    if attachments.is_empty() {
        return Ok(Vec::new());
    }
    let dir = config::data_dir().join("draft-attachments").join(draft_id.to_string());
    std::fs::create_dir_all(&dir)?;
    let mut rows = Vec::new();
    for (index, item) in attachments.iter().enumerate() {
        let path = dir.join(format!("{:02}-{}", index, sanitize_filename(&item.filename)));
        std::fs::write(&path, &item.data)?;
        rows.push(DraftAttachmentRow {
            filename: item.filename.clone(),
            mime: item.mime.clone(),
            size: item.data.len(),
            path: path.to_string_lossy().into_owned(),
        });
    }
    Ok(rows)
}

/// Copy a cached attachment into the user's Downloads folder, avoiding
/// overwriting an existing file with the same name.
pub fn download(item: &AttachmentRow) -> Result<std::path::PathBuf> {
    let downloads = std::env::var_os("XDG_DOWNLOAD_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            dirs_download()
        });
    std::fs::create_dir_all(&downloads)?;
    let name = sanitize_filename(&item.filename);
    let mut target = downloads.join(&name);
    let stem = std::path::Path::new(&name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "attachment".into());
    let extension = std::path::Path::new(&name)
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    let mut suffix = 1;
    while target.exists() {
        target = downloads.join(format!("{stem} ({suffix}){extension}"));
        suffix += 1;
    }
    std::fs::copy(&item.path, &target)
        .with_context(|| format!("saving attachment to {}", target.display()))?;
    Ok(target)
}

fn dirs_download() -> std::path::PathBuf {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    home.join("Downloads")
}

pub fn human_size(size: usize) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = size as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{size} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn mime_for_path(path: &std::path::Path) -> String {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "txt" | "md" | "log" => "text/plain",
        "html" | "htm" => "text/html",
        "csv" => "text/csv",
        "json" => "application/json",
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(sanitize_filename("  "), "attachment.bin");
    }

    #[test]
    fn formats_sizes() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KB");
    }
}

#[cfg(test)]
mod storage_tests {
    use super::*;

    #[test]
    fn downloads_without_overwriting() {
        let dir = std::env::temp_dir().join(format!("clean-download-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        unsafe { std::env::set_var("XDG_DOWNLOAD_DIR", &dir); }
        let source = dir.join("source.txt");
        std::fs::write(&source, b"payload").unwrap();
        let item = AttachmentRow {
            id: 1,
            message_id: 2,
            filename: "report.txt".into(),
            mime: "text/plain".into(),
            size: 7,
            path: source.to_string_lossy().into_owned(),
        };
        let first = download(&item).unwrap();
        let second = download(&item).unwrap();
        assert_eq!(first.file_name().unwrap(), "report.txt");
        assert_eq!(second.file_name().unwrap(), "report (1).txt");
        assert_eq!(std::fs::read(&first).unwrap(), b"payload");
        std::fs::remove_dir_all(&dir).ok();
    }
}
