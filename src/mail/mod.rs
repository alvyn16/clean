pub mod attachments;
pub mod auth;
pub mod avatars;
pub mod html;
pub mod images;
pub mod imap;
pub mod render;
pub mod cdp;
pub mod fast_render;
pub mod live_view;
pub mod oauth;
pub mod parse;
pub mod smtp;
pub mod sync;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MailAddress {
    pub name: String,
    pub email: String,
}

impl MailAddress {
    pub fn label(&self) -> String {
        if self.name.is_empty() {
            self.email.clone()
        } else {
            format!("{} <{}>", self.name, self.email)
        }
    }

    pub fn initials(&self) -> String {
        let source = if self.name.is_empty() {
            self.email.as_str()
        } else {
            self.name.as_str()
        };
        source
            .split_whitespace()
            .filter_map(|word| word.chars().next())
            .take(2)
            .collect::<String>()
            .to_uppercase()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Attachment {
    pub filename: String,
    pub mime: String,
    pub size: usize,
    /// Decoded file bytes for incoming mail.
    pub data: Vec<u8>,
}

/// A local file queued in the composer.
#[derive(Debug, Clone)]
pub struct OutgoingAttachment {
    pub filename: String,
    pub mime: String,
    pub size: usize,
    pub path: std::path::PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedMessage {
    pub message_id: Option<String>,
    pub thread_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub subject: String,
    pub from: MailAddress,
    pub to: Vec<MailAddress>,
    pub date: i64,
    pub snippet: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub has_attachments: bool,
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderRole {
    Inbox,
    Sent,
    Drafts,
    Archive,
    Spam,
    Trash,
    Custom,
}

impl FolderRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Sent => "sent",
            Self::Drafts => "drafts",
            Self::Archive => "archive",
            Self::Spam => "spam",
            Self::Trash => "trash",
            Self::Custom => "custom",
        }
    }

    pub fn from_folder_name(name: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        if lower == "inbox" || lower.contains("entrada") || lower.contains("recibidos") {
            Self::Inbox
        } else if lower.contains("sent")
            || lower.contains("enviados")
            || lower.contains("enviadas")
        {
            Self::Sent
        } else if lower.contains("draft") || lower.contains("borrador") {
            Self::Drafts
        } else if lower.contains("all mail")
            || lower.contains("archive")
            || lower.contains("todos")
            || lower.contains("archivo")
        {
            Self::Archive
        } else if lower.contains("spam")
            || lower.contains("junk")
            || lower.contains("correo no deseado")
        {
            Self::Spam
        } else if lower.contains("trash")
            || lower.contains("deleted")
            || lower.contains("bin")
            || lower.contains("papelera")
            || lower.contains("eliminados")
        {
            Self::Trash
        } else if lower.contains("starred") || lower.contains("destacados") {
            Self::Custom // favourites; leave as a label rather than a core folder
        } else {
            Self::Custom
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Draft {
    pub from: MailAddress,
    pub to: Vec<MailAddress>,
    pub cc: Vec<MailAddress>,
    pub bcc: Vec<MailAddress>,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    /// Deterministic Message-ID. When set, the same id is reused for the draft
    /// stored on the server so a resave can replace it in place.
    pub message_id: Option<String>,
    /// Local files to include in the message.
    pub attachments: Vec<OutgoingAttachment>,
}
