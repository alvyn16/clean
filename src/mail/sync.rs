use anyhow::Result;
use tracing::{info, warn};

use crate::config::{AccountConfig, Config};
use crate::db::{Db, DraftRow, FolderRow, MessageRow};
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
    let info = session.select(&folder.name)?;
    let total = info.exists;
    let unread = session.unseen_count(&folder.name).unwrap_or(0);

    if role == FolderRole::Drafts {
        let fetched = sync_drafts(session, db, account, folder, &info)?;
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
        // Drafts live in the `drafts` table; drop any legacy message rows.
        let _ = db.clear_folder_messages(&account.id, &folder.name);
        return Ok(fetched);
    }

    let cached_max = db.max_uid(&account.id, &folder.name)?;
    let last = info.uid_next.saturating_sub(1).max(cached_max);
    let first = if cached_max == 0 {
        info.uid_next.saturating_sub(limit).max(1)
    } else {
        (cached_max + 1).max(info.uid_next.saturating_sub(limit))
    };

    let mut fetched = 0;
    let mut fetched_idx = 0u32;
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

            let mut row = MessageRow {
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
            let msg_id = db.upsert_message(&row)?;
            // Fetch the full body for new messages so opening them is instant.
            if fetched_idx < 5 {
                if let Ok(raw) = session.fetch_body(&folder.name, header.uid) {
                    if let Ok(parsed) = parse::parse_full(&raw) {
                        row.id = msg_id;
                        row.snippet = parsed.snippet;
                        db.set_body(
                            msg_id,
                            Some(&parsed.body_text),
                            parsed.body_html.as_deref(),
                        )
                        .ok();
                    }
                }
            }
            fetched_idx += 1;
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

/// Import drafts from the server and forget ones deleted elsewhere.
fn sync_drafts(
    session: &mut imap::Session,
    db: &Db,
    account: &AccountConfig,
    folder: &imap::RemoteFolder,
    info: &imap::MailboxInfo,
) -> Result<u32> {
    let last = info.uid_next.saturating_sub(1);
    let mut fetched = 0;
    let mut present = Vec::new();
    if last == 0 || info.uid_validity == 0 {
        return Ok(0);
    }

    let headers = session.headers(&folder.name, 1, last)?;
    for header in headers {
        let Ok(summary) = parse::parse_headers_only(&header.raw) else {
            continue;
        };
        let Some(message_id) = summary.message_id.clone() else {
            continue;
        };
        present.push(message_id.clone());

        // Bodies are only needed for drafts this device hasn't seen yet.
        let known = db
            .draft_by_message_id(&account.id, &message_id)
            .ok()
            .flatten()
            .is_some();
        if known {
            continue;
        }
        let Ok(raw) = session.fetch_body(&folder.name, header.uid) else {
            continue;
        };
        let Ok(draft) = parse::parse_draft(&raw) else {
            continue;
        };
        let updated_at = if summary.date > 0 {
            summary.date
        } else {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        };
        db.upsert_draft(&DraftRow {
            id: 0,
            account_id: account.id.clone(),
            message_id: Some(message_id),
            to_addrs: draft.to,
            cc_addrs: draft.cc,
            bcc_addrs: draft.bcc,
            subject: draft.subject,
            body: draft.body,
            in_reply_to: draft.in_reply_to,
            references: draft.references.join(" "),
            updated_at,
            remote: true,
        })
        .ok();
        fetched += 1;
    }

    match db.prune_remote_drafts(&account.id, &present) {
        Ok(removed) if removed > 0 => {
            info!("{}: {} draft(s) removed elsewhere", folder.name, removed);
        }
        Ok(_) => {}
        Err(error) => warn!("could not reconcile drafts: {error:#}"),
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
