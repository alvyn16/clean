use std::io::Write;

use anyhow::Result;
use md5::Digest;

use crate::config;

/// Return the local cache path for a sender's avatar.
pub fn cache_path(email: &str) -> std::path::PathBuf {
    let hash = hex_md5(email.trim().to_ascii_lowercase().as_bytes());
    config::data_dir().join("avatars").join(format!("sender-{hash}.img"))
}

/// Fetch a sender avatar and cache it locally. Tries Gravatar first, then the
/// brand favicon for the sender's domain. Fails quietly if nothing is available.
pub fn fetch_sender_avatar(email: &str) -> Result<std::path::PathBuf> {
    let path = cache_path(email);
    if path.exists() {
        return Ok(path);
    }
    let normalized = email.trim().to_ascii_lowercase();
    let hash = hex_md5(normalized.as_bytes());

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let mut sources = vec![format!("https://www.gravatar.com/avatar/{hash}?d=404&s=96")];
    if let Some(domain) = email_domain(&normalized) {
        sources.push(format!("https://www.google.com/s2/favicons?domain={domain}&sz=96"));
        sources.push(format!("https://icons.duckduckgo.com/ip3/{domain}.ico"));
    }

    for url in sources {
        match client.get(&url).send() {
            Ok(response) => {
                let status = response.status();
                if status.is_success()
                    && let Ok(bytes) = response.bytes()
                        && bytes.len() > 32 {
                            if let Some(parent) = path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Ok(mut file) = std::fs::File::create(&path)
                                && file.write_all(&bytes).is_ok() {
                                    return Ok(path);
                                }
                        }
                tracing::debug!("[avatar] {} for {} -> status {}", url, email, status);
            }
            Err(error) => tracing::debug!("[avatar] {} for {} -> error {}", url, email, error),
        }
    }
    Err(anyhow::anyhow!("no avatar available for {email}"))
}

fn email_domain(email: &str) -> Option<String> {
    let domain = email.split('@').nth(1)?;
    let domain = domain.trim_end_matches(['>', ',', ' ']);
    if domain.is_empty() {
        return None;
    }
    Some(registrable_domain(domain))
}

/// Reduce a subdomain like `us.edm.uniqlo.com` to `uniqlo.com` so favicon
/// services can find the brand logo. Handles common two-part country TLDs.
fn registrable_domain(domain: &str) -> String {
    let parts: Vec<&str> = domain.split('.').collect();
    if parts.len() <= 2 {
        return domain.to_string();
    }
    let two_level_tlds = [
        "co.uk", "com.au", "co.jp", "com.br", "co.in", "com.mx", "co.nz",
        "com.sg", "com.hk", "co.za", "com.tr", "com.cn", "co.kr", "com.tw",
    ];
    let last_two = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
    let take = if two_level_tlds.contains(&last_two.as_str()) { 3 } else { 2 };
    parts[parts.len() - take..].join(".")
}

fn hex_md5(bytes: &[u8]) -> String {
    let hash = md5::Md5::digest(bytes);
    let mut out = String::with_capacity(32);
    for byte in hash {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
