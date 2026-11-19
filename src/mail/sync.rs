use anyhow::Result;
use tracing::{info, warn};

use crate::config::{AccountConfig, Config};
use crate::db::{Db, FolderRow, MessageRow};
use crate::mail::auth;
use crate::mail::imap;
use crate::mail::parse;
use crate::mail::FolderRole;

#[derive(Debug, Clone, Copy, Default)]
pub struct SyncReport {
    pub folders: u32,
    pub fetched: u32,
    pub failed: u32,
}

/// Pull recent headers for every selectable folder. Bodies are fetched lazily
/// when a message is opened, so the first sync stays quick.
pub fn sync_account(
    config: &Config,
    db: &Db,
    account: &AccountConfig,
    limit: u32,
) -> Result<SyncReport> {
    let credential = auth::credential(account, config)?;
    let mut session = imap::connect(account, &credential)?;
    let folders = session.list()?;

    let mut report = SyncReport::default();

    for folder in folders.iter().filter(|folder| folder.selectable) {
        let role = FolderRole::from_folder_name(&folder.name);
        if matches!(role, FolderRole::Custom) && folder.name != "INBOX" {
            continue;
        }

        match sync_folder(&mut session, db, account, folder, role, limit) {
            Ok(fetched) => {
                report.folders += 1;
                report.fetched += fetched;
            }
            Err(error) => {
                report.failed += 1;
                warn!("skipping {}: {error:#}", folder.name);
            }
        }
    }

    session.logout();
    info!(
        "synced {} folder(s), {} new message(s), {} failure(s)",
        report.folders, report.fetched, report.failed
    );
    Ok(report)
}

fn sync_folder(
    session: &mut imap::Session,
    db: &Db,
    account: &AccountConfig,
    folder: &imap::RemoteFolder,
    role: FolderRole,
    limit: u32,
) -> Result<u32> {
    let (total, unread) = session.status(&folder.name).unwrap_or((0, 0));
    let info = session.select(&folder.name)?;

    let cached_max = db.max_uid(&account.id, &folder.name)?;
    let last = info.uid_next.saturating_sub(1).max(cached_max);
    let first = if cached_max == 0 {
        info.uid_next.saturating_sub(limit).max(1)
    } else {
        (cached_max + 1).max(info.uid_next.saturating_sub(limit))
    };

    let mut fetched = 0;
    if first <= last && info.uid_validity != 0 {
        let headers = session.headers(&folder.name, first, last)?;
        for header in headers {
            let summary = match parse::parse_headers_only(&header.raw) {
                Ok(summary) => summary,
                Err(error) => {
                    warn!("skipping message {} in {}: {error}", header.uid, folder.name);
                    continue;
                }
            };

            let row = MessageRow {
                id: 0,
                account_id: account.id.clone(),
                folder: folder.name.clone(),
                uid: header.uid,
                message_id: summary.message_id,
                thread_id: summary.thread_id,
                subject: summary.subject,
                from_name: summary.from.name,
                from_addr: summary.from.email,
                to_addrs: summary
                    .to
                    .iter()
                    .map(|address| address.label())
                    .collect::<Vec<_>>()
                    .join(", "),
                date: summary.date,
                snippet: String::new(),
                flags: header.flags,
                has_attachments: false,
                body_text: None,
                body_html: None,
            };
            db.upsert_message(&row)?;
            fetched += 1;
        }
    }

    db.upsert_folder(&FolderRow {
        account_id: account.id.clone(),
        name: folder.name.clone(),
        role: role.as_str().to_string(),
        delimiter: folder.delimiter.clone(),
        uid_validity: info.uid_validity,
        uid_next: info.uid_next,
        total,
        unread,
    })?;

    if fetched > 0 {
        info!("{}: {} new message(s)", folder.name, fetched);
    }
    Ok(fetched)
}

/// Fetch and cache one message body.
pub fn fetch_body(
    config: &Config,
    db: &Db,
    account: &AccountConfig,
    folder: &str,
    uid: u32,
) -> Result<(String, Option<String>)> {
    let credential = auth::credential(account, config)?;
    let mut session = imap::connect(account, &credential)?;
    let raw = session.fetch_body(folder, uid)?;
    let parsed = parse::parse_full(&raw)?;
    let _ = session.mark_seen(folder, uid);
    session.logout();

    if let Ok(Some(row)) = db.find_by_uid(&account.id, folder, uid) {
        let _ = db.set_body(row.id, Some(&parsed.body_text), parsed.body_html.as_deref());
        if !row.flags.contains("\\Seen") {
            let flags = format!("{} \\Seen", row.flags).trim().to_string();
            let _ = db.set_flags(row.id, &flags);
        }
    }

    Ok((parsed.body_text, parsed.body_html))
}
