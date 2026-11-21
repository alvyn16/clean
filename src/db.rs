use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

use crate::config;

#[derive(Debug, Clone)]
pub struct FolderRow {
    pub account_id: String,
    pub name: String,
    pub role: String,
    pub delimiter: Option<String>,
    pub uid_validity: u32,
    pub uid_next: u32,
    pub total: u32,
    pub unread: u32,
}

#[derive(Debug, Clone)]
pub struct MessageRow {
    pub id: i64,
    pub account_id: String,
    pub folder: String,
    pub uid: u32,
    pub message_id: Option<String>,
    pub thread_id: Option<String>,
    pub subject: String,
    pub from_name: String,
    pub from_addr: String,
    pub to_addrs: String,
    pub date: i64,
    pub snippet: String,
    pub flags: String,
    pub has_attachments: bool,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
}

/// A draft being composed. The local `drafts` table is the source of truth for
/// the Drafts folder; `message_id` is the deterministic id used to replace the
/// copy stored on the IMAP server.
#[derive(Debug, Clone)]
pub struct DraftRow {
    pub id: i64,
    pub account_id: String,
    pub message_id: Option<String>,
    pub to_addrs: String,
    pub cc_addrs: String,
    pub bcc_addrs: String,
    pub subject: String,
    pub body: String,
    pub in_reply_to: Option<String>,
    pub references: String,
    pub updated_at: i64,
    /// True when a copy of this draft is known to exist in the IMAP Drafts
    /// folder. Server-side deletions are only reconciled for these rows.
    pub remote: bool,
}

/// An attachment decoded from a message and cached on disk.
#[derive(Debug, Clone)]
pub struct AttachmentRow {
    pub id: i64,
    pub message_id: i64,
    pub filename: String,
    pub mime: String,
    pub size: usize,
    pub path: String,
}

/// A local file attached to a draft being composed.
#[derive(Debug, Clone)]
pub struct DraftAttachmentRow {
    pub filename: String,
    pub mime: String,
    pub size: usize,
    pub path: String,
}

pub struct EventRow {
    pub id: i64,
    pub title: String,
    pub day: String,
    pub start_min: i32,
    pub end_min: i32,
    pub emoji: String,
    pub color: i32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Stats {
    pub messages: u64,
    pub unread: u64,
}

pub struct Db {
    conn: Connection,
}

/// Message-IDs are stored without angle brackets, matching `MessageRow`.
fn normalize_message_id(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_string()
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

impl Db {
    pub fn path() -> PathBuf {
        config::data_dir().join("clean.db")
    }

    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA)
            .context("initializing the local mail cache")?;
        // Older databases predate the drafts `remote` column.
        let _ = conn.execute(
            "ALTER TABLE drafts ADD COLUMN remote INTEGER NOT NULL DEFAULT 0",
            [],
        );
        Ok(Self { conn })
    }

    pub fn open_default() -> Result<Self> {
        Self::open(&Self::path())
    }

    pub fn upsert_folder(&self, folder: &FolderRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO folders
                (account_id, name, role, delimiter, uid_validity, uid_next, total, unread)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(account_id, name) DO UPDATE SET
                role = excluded.role,
                delimiter = excluded.delimiter,
                uid_validity = excluded.uid_validity,
                uid_next = excluded.uid_next,
                total = excluded.total,
                unread = excluded.unread",
            params![
                folder.account_id,
                folder.name,
                folder.role,
                folder.delimiter,
                folder.uid_validity as i64,
                folder.uid_next as i64,
                folder.total as i64,
                folder.unread as i64,
            ],
        )?;
        Ok(())
    }

    pub fn folders(&self, account_id: &str) -> Result<Vec<FolderRow>> {
        let mut statement = self.conn.prepare(
            "SELECT account_id, name, role, delimiter, uid_validity, uid_next, total, unread
             FROM folders WHERE account_id = ?1 ORDER BY name",
        )?;
        let rows = statement.query_map(params![account_id], |row| {
            Ok(FolderRow {
                account_id: row.get(0)?,
                name: row.get(1)?,
                role: row.get(2)?,
                delimiter: row.get(3)?,
                uid_validity: row.get::<_, i64>(4)? as u32,
                uid_next: row.get::<_, i64>(5)? as u32,
                total: row.get::<_, i64>(6)? as u32,
                unread: row.get::<_, i64>(7)? as u32,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn all_folders(&self) -> Result<Vec<FolderRow>> {
        let mut statement = self.conn.prepare(
            "SELECT account_id, name, role, delimiter, uid_validity, uid_next, total, unread
             FROM folders ORDER BY account_id, name",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(FolderRow {
                account_id: row.get(0)?,
                name: row.get(1)?,
                role: row.get(2)?,
                delimiter: row.get(3)?,
                uid_validity: row.get::<_, i64>(4)? as u32,
                uid_next: row.get::<_, i64>(5)? as u32,
                total: row.get::<_, i64>(6)? as u32,
                unread: row.get::<_, i64>(7)? as u32,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn max_uid(&self, account_id: &str, folder: &str) -> Result<u32> {
        let uid: i64 = self.conn.query_row(
            "SELECT coalesce(max(uid), 0) FROM messages WHERE account_id = ?1 AND folder = ?2",
            params![account_id, folder],
            |row| row.get(0),
        )?;
        Ok(uid as u32)
    }

    pub fn unread(&self, account_id: &str, folder: &str) -> Result<u64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages
             WHERE account_id = ?1 AND folder = ?2 AND instr(flags, '\\Seen') = 0",
            params![account_id, folder],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Load one message including its cached bodies.
    pub fn message(&self, id: i64) -> Result<Option<MessageRow>> {
        let mut statement = self.conn.prepare(
            "SELECT id, account_id, folder, uid, message_id, thread_id, subject, from_name,
                    from_addr, to_addrs, date, snippet, flags, has_attachments, body_text,
                    body_html
             FROM messages WHERE id = ?1",
        )?;
        let mut rows = statement.query_map(params![id], |row| {
            let mut message = message_from_row(row)?;
            message.body_text = row.get(14)?;
            message.body_html = row.get(15)?;
            Ok(message)
        })?;
        Ok(rows.next().transpose()?)
    }

    /// Find a cached message by its IMAP UID, used when fetching a body on demand.
    pub fn find_by_uid(
        &self,
        account_id: &str,
        folder: &str,
        uid: u32,
    ) -> Result<Option<MessageRow>> {
        let mut statement = self.conn.prepare(
            "SELECT id, account_id, folder, uid, message_id, thread_id, subject, from_name,
                    from_addr, to_addrs, date, snippet, flags, has_attachments
             FROM messages WHERE account_id = ?1 AND folder = ?2 AND uid = ?3 LIMIT 1",
        )?;
        let mut rows = statement.query_map(
            params![account_id, folder, uid as i64],
            message_from_row,
        )?;
        Ok(rows.next().transpose()?)
    }

    /// The newest message in a conversation, used by the reader pane.
    pub fn thread(&self, thread_id: &str, limit: u32) -> Result<Vec<MessageRow>> {
        let mut statement = self.conn.prepare(
            "SELECT id, account_id, folder, uid, message_id, thread_id, subject, from_name,
                    from_addr, to_addrs, date, snippet, flags, has_attachments
             FROM messages WHERE thread_id = ?1
             ORDER BY date ASC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![thread_id, limit as i64], message_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn set_body(&self, id: i64, text: Option<&str>, html: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET body_text = coalesce(?2, body_text), body_html = coalesce(?3, body_html)
             WHERE id = ?1",
            params![id, text, html],
        )?;
        Ok(())
    }

    pub fn set_has_attachments(&self, id: i64, value: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET has_attachments = ?2 WHERE id = ?1",
            params![id, value as i64],
        )?;
        Ok(())
    }

    /// Replace the cached attachment metadata for a message.
    pub fn set_message_attachments(&self, message_id: i64, items: &[AttachmentRow]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM attachments WHERE message_id = ?1", params![message_id])?;
        for item in items {
            tx.execute(
                "INSERT INTO attachments(message_id, filename, mime, size, path)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(message_id, filename) DO UPDATE SET
                    mime = excluded.mime, size = excluded.size, path = excluded.path",
                params![message_id, item.filename, item.mime, item.size as i64, item.path],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn attachments_for(&self, message_id: i64) -> Result<Vec<AttachmentRow>> {
        let mut statement = self.conn.prepare(
            "SELECT id, message_id, filename, mime, size, path
             FROM attachments WHERE message_id = ?1 ORDER BY id",
        )?;
        let rows = statement.query_map(params![message_id], |row| {
            Ok(AttachmentRow {
                id: row.get(0)?,
                message_id: row.get(1)?,
                filename: row.get(2)?,
                mime: row.get(3)?,
                size: row.get::<_, i64>(4)? as usize,
                path: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Replace the queued files for a draft.
    pub fn set_draft_attachments(&self, draft_id: i64, items: &[DraftAttachmentRow]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM draft_attachments WHERE draft_id = ?1", params![draft_id])?;
        for item in items {
            tx.execute(
                "INSERT INTO draft_attachments(draft_id, filename, mime, size, path)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![draft_id, item.filename, item.mime, item.size as i64, item.path],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn draft_attachments(&self, draft_id: i64) -> Result<Vec<DraftAttachmentRow>> {
        let mut statement = self.conn.prepare(
            "SELECT filename, mime, size, path FROM draft_attachments
             WHERE draft_id = ?1 ORDER BY id",
        )?;
        let rows = statement.query_map(params![draft_id], |row| {
            Ok(DraftAttachmentRow {
                filename: row.get(0)?,
                mime: row.get(1)?,
                size: row.get::<_, i64>(2)? as usize,
                path: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn set_flags(&self, id: i64, flags: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET flags = ?2 WHERE id = ?1",
            params![id, flags],
        )?;
        Ok(())
    }

    pub fn delete_message(&self, id: i64) -> Result<()> {
        let paths: Vec<String> = self
            .conn
            .prepare("SELECT path FROM attachments WHERE message_id = ?1")?
            .query_map(params![id], |row| row.get(0))?
            .filter_map(Result::ok)
            .collect();
        for path in paths {
            let _ = std::fs::remove_file(&path);
            if let Some(dir) = std::path::Path::new(&path).parent() {
                let _ = std::fs::remove_dir(dir);
            }
        }
        self.conn.execute("DELETE FROM attachments WHERE message_id = ?1", params![id])?;
        self.conn.execute("DELETE FROM messages WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Cached message ids, UIDs and flags inside a folder's UID window.
    pub fn messages_in_uid_range(
        &self,
        account_id: &str,
        folder: &str,
        first: u32,
        last: u32,
    ) -> Result<Vec<(i64, u32, String)>> {
        let mut statement = self.conn.prepare(
            "SELECT id, uid, flags FROM messages
             WHERE account_id = ?1 AND folder = ?2 AND uid BETWEEN ?3 AND ?4",
        )?;
        let rows = statement.query_map(
            params![account_id, folder, first as i64, last as i64],
            |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as u32, row.get(2)?)),
        )?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Drop every cached message row for one folder (used for the Drafts
    /// folder, whose contents live in the `drafts` table instead).
    pub fn clear_folder_messages(&self, account_id: &str, folder: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM messages WHERE account_id = ?1 AND folder = ?2",
            params![account_id, folder],
        )?;
        Ok(())
    }

    /// Relocate a cached message to a different folder/uid in place so the next
    /// sync upsert won't create a duplicate row.
    pub fn move_message(&self, id: i64, folder: &str, uid: u32) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET folder = ?2, uid = ?3 WHERE id = ?1",
            params![id, folder, uid as i64],
        )?;
        Ok(())
    }

    pub fn folder_by_name(&self, account_id: &str, name: &str) -> Result<Option<FolderRow>> {
        self.conn
            .query_row(
                "SELECT account_id, name, role, delimiter, uid_validity, uid_next, total, unread
                 FROM folders WHERE account_id = ?1 AND name = ?2",
                params![account_id, name],
                |row| {
                    Ok(FolderRow {
                        account_id: row.get(0)?,
                        name: row.get(1)?,
                        role: row.get(2)?,
                        delimiter: row.get(3)?,
                        uid_validity: row.get(4)?,
                        uid_next: row.get(5)?,
                        total: row.get(6)?,
                        unread: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn folder_by_role(&self, account_id: &str, role: &str) -> Result<Option<FolderRow>> {        self.conn
            .query_row(
                "SELECT account_id, name, role, delimiter, uid_validity, uid_next, total, unread
                 FROM folders WHERE account_id = ?1 AND role = ?2 ORDER BY name LIMIT 1",
                params![account_id, role],
                |row| {
                    Ok(FolderRow {
                        account_id: row.get(0)?,
                        name: row.get(1)?,
                        role: row.get(2)?,
                        delimiter: row.get(3)?,
                        uid_validity: row.get(4)?,
                        uid_next: row.get(5)?,
                        total: row.get(6)?,
                        unread: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    // ---- drafts ---------------------------------------------------------

    pub fn upsert_draft(&self, draft: &DraftRow) -> Result<i64> {
        let message_id = draft
            .message_id
            .as_deref()
            .map(normalize_message_id);
        if draft.id > 0 {
            self.conn.execute(
                "UPDATE drafts SET
                    message_id = ?2, to_addrs = ?3, cc_addrs = ?4, bcc_addrs = ?5,
                    subject = ?6, body = ?7, in_reply_to = ?8, \"references\" = ?9,
                    updated_at = ?10
                 WHERE id = ?1",
                params![
                    draft.id,
                    message_id,
                    draft.to_addrs,
                    draft.cc_addrs,
                    draft.bcc_addrs,
                    draft.subject,
                    draft.body,
                    draft.in_reply_to,
                    draft.references,
                    draft.updated_at,
                ],
            )?;
            return Ok(draft.id);
        }
        self.conn.execute(
            "INSERT INTO drafts
                (account_id, message_id, to_addrs, cc_addrs, bcc_addrs, subject, body,
                 in_reply_to, \"references\", updated_at, remote)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                draft.account_id,
                message_id,
                draft.to_addrs,
                draft.cc_addrs,
                draft.bcc_addrs,
                draft.subject,
                draft.body,
                draft.in_reply_to,
                draft.references,
                draft.updated_at,
                draft.remote,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn drafts(&self, account_id: &str) -> Result<Vec<DraftRow>> {
        let mut statement = self.conn.prepare(
            "SELECT id, account_id, message_id, to_addrs, cc_addrs, bcc_addrs, subject, body,
                    in_reply_to, \"references\", updated_at, remote
             FROM drafts WHERE account_id = ?1 ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map(params![account_id], |row| {
            Ok(DraftRow {
                id: row.get(0)?,
                account_id: row.get(1)?,
                message_id: row.get(2)?,
                to_addrs: row.get(3)?,
                cc_addrs: row.get(4)?,
                bcc_addrs: row.get(5)?,
                subject: row.get(6)?,
                body: row.get(7)?,
                in_reply_to: row.get(8)?,
                references: row.get(9)?,
                updated_at: row.get(10)?,
                remote: row.get::<_, i64>(11)? != 0,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn draft(&self, id: i64) -> Result<Option<DraftRow>> {
        self.conn
            .query_row(
                "SELECT id, account_id, message_id, to_addrs, cc_addrs, bcc_addrs, subject, body,
                        in_reply_to, \"references\", updated_at, remote
                 FROM drafts WHERE id = ?1",
                params![id],
                |row| {
                    Ok(DraftRow {
                        id: row.get(0)?,
                        account_id: row.get(1)?,
                        message_id: row.get(2)?,
                        to_addrs: row.get(3)?,
                        cc_addrs: row.get(4)?,
                        bcc_addrs: row.get(5)?,
                        subject: row.get(6)?,
                        body: row.get(7)?,
                        in_reply_to: row.get(8)?,
                        references: row.get(9)?,
                        updated_at: row.get(10)?,
                        remote: row.get::<_, i64>(11)? != 0,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn draft_by_message_id(
        &self,
        account_id: &str,
        message_id: &str,
    ) -> Result<Option<DraftRow>> {
        let message_id = normalize_message_id(message_id);
        self.conn
            .query_row(
                "SELECT id, account_id, message_id, to_addrs, cc_addrs, bcc_addrs, subject, body,
                        in_reply_to, \"references\", updated_at, remote
                 FROM drafts WHERE account_id = ?1 AND message_id = ?2 LIMIT 1",
                params![account_id, message_id],
                |row| {
                    Ok(DraftRow {
                        id: row.get(0)?,
                        account_id: row.get(1)?,
                        message_id: row.get(2)?,
                        to_addrs: row.get(3)?,
                        cc_addrs: row.get(4)?,
                        bcc_addrs: row.get(5)?,
                        subject: row.get(6)?,
                        body: row.get(7)?,
                        in_reply_to: row.get(8)?,
                        references: row.get(9)?,
                        updated_at: row.get(10)?,
                        remote: row.get::<_, i64>(11)? != 0,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn delete_draft(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM draft_attachments WHERE draft_id = ?1", params![id])?;
        self.conn.execute("DELETE FROM drafts WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Remember whether the IMAP Drafts folder currently holds a copy.
    pub fn mark_draft_remote(&self, id: i64, remote: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE drafts SET remote = ?2 WHERE id = ?1",
            params![id, remote],
        )?;
        Ok(())
    }

    /// Drop drafts that were previously on the server but are gone now.
    /// Drafts never uploaded (or still waiting to upload) are left alone, as
    /// are very recent ones that may be mid-upload.
    pub fn prune_remote_drafts(&self, account_id: &str, keep: &[String]) -> Result<usize> {
        let cutoff = now_unix().saturating_sub(120);
        let mut statement = self.conn.prepare(
            "SELECT id, message_id FROM drafts
             WHERE account_id = ?1 AND remote = 1 AND updated_at < ?2",
        )?;
        let stale: Vec<i64> = statement
            .query_map(params![account_id, cutoff], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            })?
            .filter_map(|row| row.ok())
            .filter(|(_, message_id)| match message_id {
                Some(id) => {
                    let normalized = normalize_message_id(id);
                    !keep
                        .iter()
                        .any(|kept| normalize_message_id(kept) == normalized)
                }
                None => true,
            })
            .map(|(id, _)| id)
            .collect();
        drop(statement);
        for id in &stale {
            self.delete_draft(*id)?;
        }
        Ok(stale.len())
    }

    pub fn upsert_message(&self, message: &MessageRow) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO messages
                (account_id, folder, uid, message_id, thread_id, subject, from_name, from_addr,
                 to_addrs, date, snippet, flags, has_attachments, body_text, body_html)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(account_id, folder, uid) DO UPDATE SET
                message_id = excluded.message_id,
                thread_id = excluded.thread_id,
                subject = excluded.subject,
                from_name = excluded.from_name,
                from_addr = excluded.from_addr,
                to_addrs = excluded.to_addrs,
                date = excluded.date,
                snippet = excluded.snippet,
                flags = excluded.flags,
                has_attachments = excluded.has_attachments,
                body_text = coalesce(excluded.body_text, messages.body_text),
                body_html = coalesce(excluded.body_html, messages.body_html)",
            params![
                message.account_id,
                message.folder,
                message.uid as i64,
                message.message_id,
                message.thread_id,
                message.subject,
                message.from_name,
                message.from_addr,
                message.to_addrs,
                message.date,
                message.snippet,
                message.flags,
                message.has_attachments as i64,
                message.body_text,
                message.body_html,
            ],
        )?;

        let id = self.conn.query_row(
            "SELECT id FROM messages WHERE account_id = ?1 AND folder = ?2 AND uid = ?3",
            params![message.account_id, message.folder, message.uid as i64],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn list_messages(
        &self,
        account_id: &str,
        folder: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MessageRow>> {
        let mut statement = self.conn.prepare(
            "SELECT id, account_id, folder, uid, message_id, thread_id, subject, from_name,
                    from_addr, to_addrs, date, snippet, flags, has_attachments
             FROM messages
             WHERE account_id = ?1 AND folder = ?2
             ORDER BY date DESC
             LIMIT ?3 OFFSET ?4",
        )?;
        let rows = statement.query_map(
            params![account_id, folder, limit as i64, offset as i64],
            message_from_row,
        )?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<MessageRow>> {
        let terms: Vec<String> = query
            .split_whitespace()
            .filter(|term| !term.is_empty())
            .map(|term| format!("\"{}\"*", term.replace('"', " ")))
            .collect();
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let fts_query = terms.join(" AND ");
        let mut statement = self.conn.prepare(
            "SELECT m.id, m.account_id, m.folder, m.uid, m.message_id, m.thread_id, m.subject,
                    m.from_name, m.from_addr, m.to_addrs, m.date, m.snippet, m.flags,
                    m.has_attachments
             FROM messages_fts
             JOIN messages m ON m.id = messages_fts.rowid
             WHERE messages_fts MATCH ?1
             ORDER BY m.date DESC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(params![fts_query, limit as i64], message_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// All locally stored calendar events, in chronological order.
    pub fn events(&self) -> Result<Vec<EventRow>> {
        let mut statement = self.conn.prepare(
            "SELECT id, title, day, start_min, end_min, emoji, color
             FROM events ORDER BY day, start_min",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(EventRow {
                id: row.get(0)?,
                title: row.get(1)?,
                day: row.get(2)?,
                start_min: row.get(3)?,
                end_min: row.get(4)?,
                emoji: row.get(5)?,
                color: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn add_event(&self, event: &EventRow) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO events (title, day, start_min, end_min, emoji, color)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.title,
                event.day,
                event.start_min,
                event.end_min,
                event.emoji,
                event.color,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_event(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM events WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn stats(&self) -> Result<Stats> {
        let messages: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))?;
        let unread: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE instr(flags, '\\Seen') = 0",
            [],
            |row| row.get(0),
        )?;
        Ok(Stats {
            messages: messages as u64,
            unread: unread as u64,
        })
    }

    pub fn count_messages(&self) -> Result<u64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    /// Delete cached mail for one account, used when disconnecting.
    pub fn clear_account(&self, account_id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM messages WHERE account_id = ?1", params![account_id])?;
        self.conn.execute("DELETE FROM folders WHERE account_id = ?1", params![account_id])?;
        self.conn.execute("DELETE FROM drafts WHERE account_id = ?1", params![account_id])?;
        Ok(())
    }
}

fn message_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MessageRow> {
    Ok(MessageRow {
        id: row.get(0)?,
        account_id: row.get(1)?,
        folder: row.get(2)?,
        uid: row.get::<_, i64>(3)? as u32,
        message_id: row.get(4)?,
        thread_id: row.get(5)?,
        subject: row.get(6)?,
        from_name: row.get(7)?,
        from_addr: row.get(8)?,
        to_addrs: row.get(9)?,
        date: row.get(10)?,
        snippet: row.get(11)?,
        flags: row.get(12)?,
        has_attachments: row.get::<_, i64>(13)? != 0,
        body_text: None,
        body_html: None,
    })
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS folders (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id   TEXT NOT NULL,
    name         TEXT NOT NULL,
    role         TEXT NOT NULL,
    delimiter    TEXT,
    uid_validity INTEGER NOT NULL DEFAULT 0,
    uid_next     INTEGER NOT NULL DEFAULT 0,
    total        INTEGER NOT NULL DEFAULT 0,
    unread       INTEGER NOT NULL DEFAULT 0,
    UNIQUE(account_id, name)
);

CREATE TABLE IF NOT EXISTS messages (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id      TEXT NOT NULL,
    folder          TEXT NOT NULL,
    uid             INTEGER NOT NULL,
    message_id      TEXT,
    thread_id       TEXT,
    subject         TEXT NOT NULL DEFAULT '',
    from_name       TEXT NOT NULL DEFAULT '',
    from_addr       TEXT NOT NULL DEFAULT '',
    to_addrs        TEXT NOT NULL DEFAULT '',
    date            INTEGER NOT NULL DEFAULT 0,
    snippet         TEXT NOT NULL DEFAULT '',
    flags           TEXT NOT NULL DEFAULT '',
    has_attachments INTEGER NOT NULL DEFAULT 0,
    body_text       TEXT,
    body_html       TEXT,
    UNIQUE(account_id, folder, uid)
);

CREATE INDEX IF NOT EXISTS idx_messages_folder_date
    ON messages(account_id, folder, date DESC);
CREATE INDEX IF NOT EXISTS idx_messages_message_id
    ON messages(message_id);

CREATE TABLE IF NOT EXISTS drafts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id  TEXT NOT NULL,
    message_id  TEXT,
    to_addrs    TEXT NOT NULL DEFAULT '',
    cc_addrs    TEXT NOT NULL DEFAULT '',
    bcc_addrs   TEXT NOT NULL DEFAULT '',
    subject     TEXT NOT NULL DEFAULT '',
    body        TEXT NOT NULL DEFAULT '',
    in_reply_to TEXT,
    "references" TEXT NOT NULL DEFAULT '',
    updated_at  INTEGER NOT NULL DEFAULT 0,
    remote      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_drafts_account_updated
    ON drafts(account_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS attachments (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id  INTEGER NOT NULL,
    filename    TEXT NOT NULL,
    mime        TEXT NOT NULL DEFAULT 'application/octet-stream',
    size        INTEGER NOT NULL DEFAULT 0,
    path        TEXT NOT NULL,
    UNIQUE(message_id, filename)
);
CREATE INDEX IF NOT EXISTS idx_attachments_message
    ON attachments(message_id);

CREATE TABLE IF NOT EXISTS draft_attachments (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    draft_id    INTEGER NOT NULL,
    filename    TEXT NOT NULL,
    mime        TEXT NOT NULL DEFAULT 'application/octet-stream',
    size        INTEGER NOT NULL DEFAULT 0,
    path        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_draft_attachments_draft
    ON draft_attachments(draft_id);

CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    subject,
    from_name,
    from_addr,
    snippet,
    body_text,
    content='messages',
    content_rowid='id'
);

CREATE TABLE IF NOT EXISTS events (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    title        TEXT NOT NULL,
    day          TEXT NOT NULL,
    start_min    INTEGER NOT NULL DEFAULT 0,
    end_min      INTEGER NOT NULL DEFAULT 0,
    emoji        TEXT NOT NULL DEFAULT '',
    color        INTEGER NOT NULL DEFAULT 0
);

CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, subject, from_name, from_addr, snippet, body_text)
    VALUES (new.id, new.subject, new.from_name, new.from_addr, new.snippet,
            coalesce(new.body_text, ''));
END;

CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, subject, from_name, from_addr, snippet, body_text)
    VALUES ('delete', old.id, old.subject, old.from_name, old.from_addr, old.snippet,
            coalesce(old.body_text, ''));
END;

CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, subject, from_name, from_addr, snippet, body_text)
    VALUES ('delete', old.id, old.subject, old.from_name, old.from_addr, old.snippet,
            coalesce(old.body_text, ''));
    INSERT INTO messages_fts(rowid, subject, from_name, from_addr, snippet, body_text)
    VALUES (new.id, new.subject, new.from_name, new.from_addr, new.snippet,
            coalesce(new.body_text, ''));
END;
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> (Db, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("test.db")).unwrap();
        (db, dir)
    }

    #[test]
    fn stores_and_searches_messages() {
        let (db, _dir) = temp_db();
        db.upsert_folder(&FolderRow {
            account_id: "a".into(),
            name: "INBOX".into(),
            role: "inbox".into(),
            delimiter: Some("/".into()),
            uid_validity: 1,
            uid_next: 10,
            total: 1,
            unread: 1,
        })
        .unwrap();

        let message = MessageRow {
            id: 0,
            account_id: "a".into(),
            folder: "INBOX".into(),
            uid: 7,
            message_id: Some("<one@example>".into()),
            thread_id: Some("<one@example>".into()),
            subject: "Saturday morning walk".into(),
            from_name: "Dog Park Crew".into(),
            from_addr: "crew@example.com".into(),
            to_addrs: "me@example.com".into(),
            date: 1_760_000_000,
            snippet: "Canyon loop at 8, coffee after.".into(),
            flags: String::new(),
            has_attachments: false,
            body_text: Some("Canyon loop at 8, coffee after.".into()),
            body_html: None,
        };
        db.upsert_message(&message).unwrap();

        let found = db.search("canyon", 10).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].subject, "Saturday morning walk");
    }

    #[test]
    fn stores_local_events() {
        let (db, _dir) = temp_db();
        db.add_event(&EventRow {
            id: 0,
            title: "Morning walk".into(),
            day: "2026-11-22".into(),
            start_min: 8 * 60,
            end_min: 9 * 60,
            emoji: "🌿".into(),
            color: 2,
        })
        .unwrap();

        let events = db.events().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Morning walk");
        assert_eq!(events[0].color, 2);

        db.delete_event(events[0].id).unwrap();
        assert!(db.events().unwrap().is_empty());
    }
}

#[cfg(test)]
mod action_tests {
    use super::*;

    #[test]
    fn delete_and_move_message() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("test.db")).unwrap();
        let account_id = "test-account";
        db.upsert_folder(&FolderRow {
            account_id: account_id.into(),
            name: "INBOX".into(),
            role: "inbox".into(),
            delimiter: None,
            uid_validity: 1,
            uid_next: 2,
            total: 1,
            unread: 1,
        }).unwrap();
        db.upsert_folder(&FolderRow {
            account_id: account_id.into(),
            name: "[Gmail]/Trash".into(),
            role: "trash".into(),
            delimiter: None,
            uid_validity: 1,
            uid_next: 1,
            total: 0,
            unread: 0,
        }).unwrap();
        let id = db.upsert_message(&MessageRow {
            id: 0,
            account_id: account_id.into(),
            folder: "INBOX".into(),
            uid: 42,
            message_id: None,
            thread_id: None,
            subject: "Test".into(),
            from_name: String::new(),
            from_addr: "a@b.com".into(),
            to_addrs: String::new(),
            date: 0,
            snippet: String::new(),
            flags: "\\Seen".into(),
            has_attachments: false,
            body_text: None,
            body_html: None,
        }).unwrap();

        // folder lookup by role/name
        assert_eq!(db.folder_by_role(account_id, "trash").unwrap().unwrap().name, "[Gmail]/Trash");
        assert_eq!(db.folder_by_name(account_id, "INBOX").unwrap().unwrap().role, "inbox");

        // move in place
        db.move_message(id, "[Gmail]/Trash", 99).unwrap();
        let moved = db.message(id).unwrap().unwrap();
        assert_eq!(moved.folder, "[Gmail]/Trash");
        assert_eq!(moved.uid, 99);
        assert!(db.find_by_uid(account_id, "INBOX", 42).unwrap().is_none());
        assert!(db.find_by_uid(account_id, "[Gmail]/Trash", 99).unwrap().is_some());

        // delete
        db.delete_message(id).unwrap();
        assert!(db.message(id).unwrap().is_none());
    }
}

#[cfg(test)]
mod draft_tests {
    use super::*;

    fn row(id: i64, body: &str, updated_at: i64) -> DraftRow {
        DraftRow {
            id,
            account_id: "a".into(),
            message_id: Some("<draft@clean.local>".into()),
            to_addrs: "friend@example.com".into(),
            cc_addrs: String::new(),
            bcc_addrs: String::new(),
            subject: "Hello".into(),
            body: body.into(),
            in_reply_to: None,
            references: String::new(),
            updated_at,
            remote: false,
        }
    }

    #[test]
    fn saves_updates_and_deletes_drafts() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("test.db")).unwrap();

        let id = db.upsert_draft(&row(0, "first", 10)).unwrap();
        assert!(id > 0);
        let listed = db.drafts("a").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].body, "first");

        // Updating an existing row replaces it instead of inserting a copy.
        db.upsert_draft(&row(id, "second", 20)).unwrap();
        let listed = db.drafts("a").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].body, "second");
        assert_eq!(listed[0].updated_at, 20);

        assert!(db
            .draft_by_message_id("a", "<draft@clean.local>")
            .unwrap()
            .is_some());
        assert!(db.draft(id).unwrap().is_some());

        db.delete_draft(id).unwrap();
        assert!(db.drafts("a").unwrap().is_empty());
    }

    #[test]
    fn only_reconciles_drafts_that_reached_the_server() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("test.db")).unwrap();

        // A local-only draft is never pruned, even though it has no server copy.
        let local = db.upsert_draft(&row(0, "local only", 10)).unwrap();
        assert_eq!(db.prune_remote_drafts("a", &[]).unwrap(), 0);
        assert!(db.draft(local).unwrap().is_some());

        // Once uploaded it becomes reconcilable.
        db.mark_draft_remote(local, true).unwrap();
        assert!(db.draft(local).unwrap().unwrap().remote);
        assert_eq!(
            db.prune_remote_drafts("a", &["draft@clean.local".to_string()])
                .unwrap(),
            0
        );
        assert!(db.draft(local).unwrap().is_some());
        assert_eq!(db.prune_remote_drafts("a", &[]).unwrap(), 1);
        assert!(db.draft(local).unwrap().is_none());
    }

    #[test]
    fn stores_draft_attachments() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("test.db")).unwrap();
        let id = db.upsert_draft(&DraftRow {
            id: 0,
            account_id: "a".into(),
            message_id: Some("m@clean.local".into()),
            to_addrs: "to@example.com".into(),
            cc_addrs: String::new(),
            bcc_addrs: String::new(),
            subject: "with file".into(),
            body: "body".into(),
            in_reply_to: None,
            references: String::new(),
            updated_at: 1,
            remote: false,
        }).unwrap();
        db.set_draft_attachments(id, &[DraftAttachmentRow {
            filename: "note.txt".into(),
            mime: "text/plain".into(),
            size: 4,
            path: "/tmp/note.txt".into(),
        }]).unwrap();
        let rows = db.draft_attachments(id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].filename, "note.txt");
        db.delete_draft(id).unwrap();
        assert!(db.draft_attachments(id).unwrap().is_empty());
    }
}
