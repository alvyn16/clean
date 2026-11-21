use crate::ui::model::UiMessage;
use crate::ui::parse_addresses;

/// Build To/Cc/Bcc for reply-all: original sender and other To recipients in
/// To, original Cc recipients in Cc, removing our own address and duplicates.
pub fn reply_recipients(message: &UiMessage, own: &str) -> (String, String, String) {
    let email_of = |part: &str| {
        parse_addresses(part)
            .first()
            .map(|address| address.email.to_ascii_lowercase())
            .unwrap_or_else(|| part.trim().to_ascii_lowercase())
    };
    let own_lower = own.trim().to_ascii_lowercase();
    let sender_lower = message.row.from_addr.to_ascii_lowercase();
    let mut seen = std::collections::HashSet::new();
    if !sender_lower.is_empty() {
        seen.insert(sender_lower);
    }
    let mut to: Vec<String> = Vec::new();
    let mut cc: Vec<String> = Vec::new();
    for part in message.row.to_addrs.split([',', ';']).map(str::trim) {
        let email = email_of(part);
        if part.is_empty() || email.is_empty() || email == own_lower || !seen.insert(email) {
            continue;
        }
        to.push(part.to_string());
    }
    for part in message.row.cc_addrs.split([',', ';']).map(str::trim) {
        let email = email_of(part);
        if part.is_empty() || email.is_empty() || email == own_lower || !seen.insert(email) {
            continue;
        }
        cc.push(part.to_string());
    }
    (
        std::iter::once(format!("{} <{}>", message.sender(), message.row.from_addr))
            .chain(to)
            .collect::<Vec<_>>()
            .join(", "),
        cc.join(", "),
        String::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(from: &str, to: &str, cc: &str) -> UiMessage {
        let mut msg = UiMessage::from_row(crate::db::MessageRow {
            id: 1,
            account_id: "a".into(),
            folder: "INBOX".into(),
            uid: 1,
            message_id: Some("m@clean.local".into()),
            thread_id: None,
            subject: "Subject".into(),
            from_name: String::new(),
            from_addr: from.into(),
            to_addrs: to.into(),
            cc_addrs: cc.into(),
            date: 0,
            snippet: String::new(),
            flags: String::new(),
            has_attachments: false,
            body_text: None,
            body_html: None,
        });
        msg.row.from_name = "Sender".into();
        msg
    }

    #[test]
    fn excludes_self_and_deduplicates() {
        let msg = message(
            "sender@example.com",
            "me@example.com, Bob <bob@example.com>, sender@example.com",
            "carol@example.com, bob@example.com, ME@example.com",
        );
        let (to, cc, _) = reply_recipients(&msg, "me@example.com");
        assert!(to.contains("sender@example.com"));
        assert!(to.contains("bob@example.com"));
        assert!(!to.to_ascii_lowercase().contains("me@example.com"));
        assert!(cc.contains("carol@example.com"));
        assert!(!cc.contains("bob@example.com"));
        assert!(!cc.to_ascii_lowercase().contains("me@example.com"));
    }
}
