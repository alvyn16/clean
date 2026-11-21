use std::collections::HashMap;

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
    /// Messages removed from the cache because the server no longer has them.
    pub removed: u32,
    /// Cached flag changes mirrored from the server.
    pub updated: u32,
}

fn normalize_flags(flags: &str) -> String {
    let mut parts: Vec<&str> = flags.split_whitespace().collect();
    parts.sort_unstable();
    parts.dedup();
    parts.join(" ")
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
            Ok(folder_report) => {
                report.folders += 1;
                report.fetched += folder_report.0;
                report.removed += folder_report.1;
                report.updated += folder_report.2;
            }
            Err(error) => {
                report.failed += 1;
                warn!("skipping {}: {error:#}", folder.name);
            }
        }
    }

    session.logout();
    info!(
        "synced {} folder(s), {} new, {} removed, {} flags updated, {} failure(s)",
        report.folders, report.fetched, report.removed, report.updated, report.failed
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
) -> Result<(u32, u32, u32)> {
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
        return Ok((fetched, 0, 0));
    }

    let mut cached_max = db.max_uid(&account.id, &folder.name)?;
    // A changed UIDVALIDITY invalidates every cached UID in the folder.
    let validity_changed = db
        .folder_by_name(&account.id, &folder.name)
        .ok()
        .flatten()
        .is_some_and(|cached| {
            cached.uid_validity != 0
                && info.uid_validity != 0
                && cached.uid_validity != info.uid_validity
        });
    if validity_changed {
        db.clear_folder_messages(&account.id, &folder.name)?;
        cached_max = 0;
    }
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
                        row.snippet = parsed.snippet.clone();
                        store_parsed(db, msg_id, &parsed);
                    }
                }
            }
            fetched_idx += 1;
            fetched += 1;
        }
    }

    let (removed, updated) = reconcile_cache(session, db, account, &folder.name, &info, validity_changed);

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

    if fetched > 0 || removed > 0 || updated > 0 {
        info!(
            "{}: {} new, {} removed, {} flags updated",
            folder.name, fetched, removed, updated
        );
    }
    Ok((fetched, removed, updated))
}

/// Bring cached UIDs and flags in line with the server. Only the UIDs we
/// actually cache are queried, so this stays cheap even for a 10k-message
/// mailbox while still catching every expunge and flag change in the cache.
fn reconcile_cache(
    session: &mut imap::Session,
    db: &Db,
    account: &AccountConfig,
    folder_name: &str,
    info: &imap::MailboxInfo,
    validity_changed: bool,
) -> (u32, u32) {
    if info.uid_validity == 0 {
        return (0, 0);
    }
    if validity_changed {
        // Everything was already dropped before the header fetch.
        return (0, 0);
    }
    let cached = db
        .messages_in_uid_range(&account.id, folder_name, 1, u32::MAX)
        .unwrap_or_default();
    if cached.is_empty() {
        return (0, 0);
    }
    let mut uids: Vec<u32> = cached.iter().map(|(_, uid, _)| *uid).collect();
    uids.sort_unstable();
    uids.dedup();

    let mut removed = 0;
    let mut updated = 0;
    match session.uids_and_flags_for(folder_name, &uids) {
        Ok(remote) => {
            let server: HashMap<u32, String> = remote.into_iter().collect();
            for (id, uid, local_flags) in cached {
                match server.get(&uid) {
                    None => {
                        let _ = db.delete_message(id);
                        removed += 1;
                    }
                    Some(flags) if normalize_flags(&local_flags) != normalize_flags(flags) => {
                        let _ = db.set_flags(id, flags);
                        updated += 1;
                    }
                    _ => {}
                }
            }
        }
        Err(error) => warn!("could not reconcile cached UIDs in {folder_name}: {error:#}"),
    }
    (removed, updated)
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
        if let Ok(draft_id) = db.upsert_draft(&DraftRow {
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
        }) {
            let parsed = parse::parse_attachments(&raw);
            if !parsed.is_empty() {
                if let Ok(items) =
                    crate::mail::attachments::save_draft_attachments(draft_id, &parsed)
                {
                    let _ = db.set_draft_attachments(draft_id, &items);
                }
            }
            fetched += 1;
        }
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

/// Cache a parsed body together with its attachments. Both the sync prefetch
/// and the on-demand fetch go through here so attachments never get lost.
fn store_parsed(db: &Db, id: i64, parsed: &crate::mail::ParsedMessage) {
    let _ = db.set_body(id, Some(&parsed.body_text), parsed.body_html.as_deref());
    let _ = db.set_has_attachments(id, !parsed.attachments.is_empty());
    if parsed.attachments.is_empty() {
        return;
    }
    match crate::mail::attachments::save_message_attachments(id, &parsed.attachments) {
        Ok(items) => {
            if let Err(error) = db.set_message_attachments(id, &items) {
                warn!("could not index attachments: {error:#}");
            }
        }
        Err(error) => warn!("could not store attachments: {error:#}"),
    }
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

    let body_text = parsed.body_text.clone();
    let body_html = parsed.body_html.clone();
    if let Ok(Some(row)) = db.find_by_uid(&account.id, folder, uid) {
        store_parsed(db, row.id, &parsed);
        if !row.flags.contains("\\Seen") {
            let flags = format!("{} \\Seen", row.flags).trim().to_string();
            let _ = db.set_flags(row.id, &flags);
        }
    }

    Ok((body_text, body_html))
}

#[cfg(test)]
mod reconciliation_tests {
    use super::*;

    #[test]
    fn normalizes_flag_order() {
        assert_eq!(normalize_flags(r"\Seen \Flagged \Seen"), r"\Flagged \Seen");
        assert_eq!(normalize_flags(""), "");
    }


    #[test]
    #[ignore]
    fn reconciles_deletions_and_flags() {
        let config = Config::load().expect("config");
        let account = config.accounts.first().expect("account").clone();
        let sender = config.accounts.get(1).cloned().unwrap_or_else(|| account.clone());
        let sender_credential = auth::credential(&sender, &config).expect("sender auth");
        let secret = match &sender_credential {
            crate::mail::imap::Credential::Password(v) => v.clone(),
            crate::mail::imap::Credential::OAuth(v) => v.clone(),
        };
        let id = format!(
            "clean-reconcile-{}@clean.local",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let from = crate::mail::MailAddress {
            name: sender.display_name.clone(),
            email: sender.email.clone(),
        };
        let draft = crate::mail::Draft {
            from: from.clone(),
            to: vec![crate::mail::MailAddress {
                name: account.display_name.clone(),
                email: account.email.clone(),
            }],
            subject: "clean reconcile test".into(),
            body_text: "temporary".into(),
            message_id: Some(id.clone()),
            ..Default::default()
        };
        let db = Db::open_default().expect("db");
        // Clear leftovers from earlier runs.
        let stale: Vec<(i64, String)> = db
            .messages_in_uid_range(&account.id, "INBOX", 1, u32::MAX)
            .unwrap()
            .into_iter()
            .filter_map(|(row_id, _, _)| {
                db.message(row_id).ok().flatten().and_then(|m| {
                    if m.subject == "clean reconcile test" {
                        m.message_id.map(|mid| (row_id, mid))
                    } else {
                        None
                    }
                })
            })
            .collect();
        if !stale.is_empty() {
            let cred = auth::credential(&account, &config).unwrap();
            let mut session = imap::connect(&account, &cred).unwrap();
            for (row_id, mid) in stale {
                let _ = session.delete_by_message_id("INBOX", &mid);
                let _ = db.delete_message(row_id);
            }
            session.logout();
        }
        crate::mail::smtp::send(&sender, &secret, &draft).expect("deliver message");
        std::thread::sleep(std::time::Duration::from_secs(2));

        sync_account(&config, &db, &account, 50).expect("initial sync");
        let row = db
            .messages_in_uid_range(&account.id, "INBOX", 1, u32::MAX)
            .unwrap()
            .into_iter()
            .find_map(|(db_id, uid, _)| {
                db.message(db_id)
                    .ok()
                    .flatten()
                    .filter(|m| m.message_id.as_deref() == Some(id.as_str()))
                    .map(|m| (m.id, uid))
            })
            .expect("imported test message");

        let credential = auth::credential(&account, &config).expect("credential");
        let mut session = imap::connect(&account, &credential).expect("connect");
        session.uid_store("INBOX", row.1, "+FLAGS (\\Flagged)").expect("flag message");
        sync_account(&config, &db, &account, 50).expect("flag sync");
        assert!(db.message(row.0).unwrap().unwrap().flags.contains("\\Flagged"));

        session.uid_store("INBOX", row.1, "-FLAGS (\\Flagged)").expect("unflag message");
        sync_account(&config, &db, &account, 50).expect("unflag sync");
        assert!(!db.message(row.0).unwrap().unwrap().flags.contains("\\Flagged"));

        session.delete_by_message_id("INBOX", &id).expect("delete test message");
        session.logout();
        sync_account(&config, &db, &account, 50).expect("delete sync");
        assert!(db.message(row.0).unwrap().is_none(), "expunged message still cached");
        println!("reconciled deletion and flag changes");
    }
}
