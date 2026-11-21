use anyhow::Result;
use mailparse::{
    parse_headers, parse_mail, DispositionType, MailAddr, MailHeaderMap, ParsedMail,
};

use crate::mail::{Attachment, MailAddress, ParsedMessage};

#[derive(Debug, Clone, Default)]
pub struct HeaderSummary {
    pub message_id: Option<String>,
    pub thread_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub subject: String,
    pub from: MailAddress,
    pub to: Vec<MailAddress>,
    pub cc: Vec<MailAddress>,
    pub date: i64,
    pub snippet: String,
}

pub fn parse_headers_only(raw: &[u8]) -> Result<HeaderSummary> {
    let (headers, _) = parse_headers(raw)?;
    let subject = headers.get_first_value("Subject").unwrap_or_default();
    let from_value = headers.get_first_value("From").unwrap_or_default();
    let to_value = headers.get_first_value("To").unwrap_or_default();
    let cc_value = headers.get_first_value("Cc").unwrap_or_default();
    let date = headers
        .get_first_value("Date")
        .and_then(|value| mailparse::dateparse(&value).ok())
        .unwrap_or(0);
    let message_id = headers
        .get_first_value("Message-ID")
        .map(|value| clean_id(&value));
    let references = headers
        .get_first_value("References")
        .map(|value| parse_ids(&value))
        .unwrap_or_default();
    let in_reply_to = headers
        .get_first_value("In-Reply-To")
        .map(|value| parse_ids(&value))
        .unwrap_or_default()
        .into_iter()
        .next();

    let thread_id = references
        .first()
        .cloned()
        .or_else(|| in_reply_to.clone())
        .or_else(|| message_id.clone());

    Ok(HeaderSummary {
        message_id,
        thread_id,
        in_reply_to,
        references,
        subject,
        from: parse_address(&from_value),
        to: parse_addresses(&to_value),
        cc: parse_addresses(&cc_value),
        date,
        snippet: String::new(),
    })
}

pub fn parse_full(raw: &[u8]) -> Result<ParsedMessage> {
    let parsed = parse_mail(raw)?;
    let (headers, _) = parse_headers(raw)?;

    let subject = headers
        .get_first_value("Subject")
        .unwrap_or_default();
    let from_value = headers.get_first_value("From").unwrap_or_default();
    let to_value = headers.get_first_value("To").unwrap_or_default();
    let date = headers
        .get_first_value("Date")
        .and_then(|value| mailparse::dateparse(&value).ok())
        .unwrap_or(0);
    let message_id = headers
        .get_first_value("Message-ID")
        .map(|value| clean_id(&value));
    let references = headers
        .get_first_value("References")
        .map(|value| parse_ids(&value))
        .unwrap_or_default();
    let in_reply_to = headers
        .get_first_value("In-Reply-To")
        .map(|value| parse_ids(&value))
        .unwrap_or_default()
        .into_iter()
        .next();
    let thread_id = references
        .first()
        .cloned()
        .or_else(|| in_reply_to.clone())
        .or_else(|| message_id.clone());

    let body_html = find_part(&parsed, "text/html").and_then(|part| part.get_body().ok());
    let body_text = find_part(&parsed, "text/plain")
        .and_then(|part| part.get_body().ok())
        .or_else(|| {
            body_html
                .as_deref()
                .and_then(|html| html2text::from_read(html.as_bytes(), 90).ok())
        })
        .unwrap_or_default();

    let mut attachments = Vec::new();
    collect_attachments(&parsed, &mut attachments);

    Ok(ParsedMessage {
        message_id,
        thread_id,
        in_reply_to,
        references,
        subject,
        from: parse_address(&from_value),
        to: parse_addresses(&to_value),
        date,
        snippet: make_snippet(&body_text),
        body_text,
        body_html,
        has_attachments: !attachments.is_empty(),
        attachments,
    })
}

/// Pull attachment parts out of a raw message without parsing compose fields.
pub fn parse_attachments(raw: &[u8]) -> Vec<Attachment> {
    let Ok(parsed) = parse_mail(raw) else {
        return Vec::new();
    };
    let mut attachments = Vec::new();
    collect_attachments(&parsed, &mut attachments);
    attachments
}

#[derive(Debug, Clone, Default)]
pub struct ParsedDraft {
    pub message_id: Option<String>,
    pub to: String,
    pub cc: String,
    pub bcc: String,
    pub subject: String,
    pub body: String,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
}

/// Parse a message stored in the Drafts folder back into compose fields.
pub fn parse_draft(raw: &[u8]) -> Result<ParsedDraft> {
    let parsed = parse_mail(raw)?;
    let (headers, _) = parse_headers(raw)?;

    let header = |name: &str| headers.get_first_value(name).unwrap_or_default();
    let subject = header("Subject");
    let message_id = headers
        .get_first_value("Message-ID")
        .map(|value| clean_id(&value))
        .filter(|value| !value.is_empty());
    let references = headers
        .get_first_value("References")
        .map(|value| parse_ids(&value))
        .unwrap_or_default();
    let in_reply_to = headers
        .get_first_value("In-Reply-To")
        .map(|value| parse_ids(&value))
        .unwrap_or_default()
        .into_iter()
        .next();

    let body = find_part(&parsed, "text/plain")
        .and_then(|part| part.get_body().ok())
        .or_else(|| {
            find_part(&parsed, "text/html")
                .and_then(|part| part.get_body().ok())
                .and_then(|html| html2text::from_read(html.as_bytes(), 90).ok())
        })
        .unwrap_or_default();

    Ok(ParsedDraft {
        message_id,
        to: header("To"),
        cc: header("Cc"),
        bcc: header("Bcc"),
        subject,
        body,
        in_reply_to,
        references,
    })
}

fn find_part<'a>(mail: &'a ParsedMail<'a>, mime: &str) -> Option<&'a ParsedMail<'a>> {    if mail.ctype.mimetype.eq_ignore_ascii_case(mime) {
        return Some(mail);
    }
    mail.subparts
        .iter()
        .find_map(|part| find_part(part, mime))
}

fn collect_attachments(mail: &ParsedMail<'_>, out: &mut Vec<Attachment>) {
    let disposition = mail.get_content_disposition();
    let filename = disposition
        .params
        .get("filename")
        .cloned()
        .or_else(|| mail.ctype.params.get("name").cloned());
    let is_attachment = matches!(disposition.disposition, DispositionType::Attachment)
        || (filename.is_some() && !mail.ctype.mimetype.starts_with("text/"));

    if is_attachment {
        if let Some(filename) = filename {
            let data = mail.get_body_raw().unwrap_or_default();
            out.push(Attachment {
                size: data.len(),
                filename,
                mime: mail.ctype.mimetype.clone(),
                data,
            });
        }
    }

    for part in &mail.subparts {
        collect_attachments(part, out);
    }
}

fn parse_address(value: &str) -> MailAddress {
    parse_addresses(value)
        .into_iter()
        .next()
        .unwrap_or_default()
}

fn parse_addresses(value: &str) -> Vec<MailAddress> {
    let Ok(list) = mailparse::addrparse(value) else {
        return Vec::new();
    };

    let mut addresses = Vec::new();
    for address in list.iter() {
        match address {
            MailAddr::Single(info) => addresses.push(MailAddress {
                name: info.display_name.clone().unwrap_or_default(),
                email: info.addr.clone(),
            }),
            MailAddr::Group(group) => {
                for info in &group.addrs {
                    addresses.push(MailAddress {
                        name: info.display_name.clone().unwrap_or_default(),
                        email: info.addr.clone(),
                    });
                }
            }
        }
    }
    addresses
}

fn parse_ids(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .map(clean_id)
        .filter(|id| !id.is_empty())
        .collect()
}

fn clean_id(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_string()
}

fn make_snippet(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut snippet = collapsed.chars().take(180).collect::<String>();
    if collapsed.chars().count() > 180 {
        snippet.push('…');
    }
    snippet
}

#[cfg(test)]
mod draft_parse_tests {
    use super::*;

    #[test]
    fn parses_a_draft_back_into_compose_fields() {
        let raw = concat!(
            "From: Me <me@example.com>\r\n",
            "To: Friend <friend@example.com>\r\n",
            "Cc: Other <other@example.com>\r\n",
            "Subject: Lunch?\r\n",
            "Message-ID: <abc@clean.local>\r\n",
            "In-Reply-To: <parent@example.com>\r\n",
            "\r\n",
            "Are you free at noon?\r\n"
        )
        .as_bytes();
        let draft = parse_draft(raw).unwrap();
        assert_eq!(draft.message_id.as_deref(), Some("abc@clean.local"));
        assert!(draft.to.contains("friend@example.com"));
        assert!(draft.cc.contains("other@example.com"));
        assert_eq!(draft.subject, "Lunch?");
        assert!(draft.body.contains("free at noon"));
        assert_eq!(draft.in_reply_to.as_deref(), Some("parent@example.com"));
    }
}

#[cfg(test)]
mod attachment_tests {
    use super::*;

    #[test]
    fn decodes_binary_attachments() {
        // "hi" in base64.
        let raw = concat!(
            "From: Me <me@example.com>\r\n",
            "Subject: file\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/mixed; boundary=\"b\"\r\n",
            "\r\n",
            "--b\r\n",
            "Content-Type: text/plain\r\n",
            "\r\n",
            "see attached\r\n",
            "--b\r\n",
            "Content-Type: application/octet-stream\r\n",
            "Content-Disposition: attachment; filename=\"note.txt\"\r\n",
            "Content-Transfer-Encoding: base64\r\n",
            "\r\n",
            "aGk=\r\n",
            "--b--\r\n"
        )
        .as_bytes();
        let parsed = parse_full(raw).unwrap();
        assert!(parsed.has_attachments);
        assert_eq!(parsed.attachments.len(), 1);
        assert_eq!(parsed.attachments[0].filename, "note.txt");
        assert_eq!(parsed.attachments[0].data, b"hi");
        assert_eq!(parsed.attachments[0].size, 2);
        assert_eq!(parse_attachments(raw).len(), 1);
    }
}

#[cfg(test)]
mod cc_parse_tests {
    use super::*;

    #[test]
    fn parses_cc_from_headers() {
        let raw = concat!(
            "From: A <a@example.com>\r\n",
            "To: B <b@example.com>\r\n",
            "Cc: C <c@example.com>, D <d@example.com>\r\n",
            "Subject: hi\r\n",
            "\r\n",
            "body\r\n"
        )
        .as_bytes();
        let summary = parse_headers_only(raw).unwrap();
        let cc: Vec<_> = summary.cc.iter().map(|a| a.email.as_str()).collect();
        assert_eq!(cc, vec!["c@example.com", "d@example.com"]);
    }
}
