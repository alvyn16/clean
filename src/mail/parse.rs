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
    pub date: i64,
    pub snippet: String,
}

pub fn parse_headers_only(raw: &[u8]) -> Result<HeaderSummary> {
    let (headers, _) = parse_headers(raw)?;
    let subject = headers.get_first_value("Subject").unwrap_or_default();
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

    Ok(HeaderSummary {
        message_id,
        thread_id,
        in_reply_to,
        references,
        subject: decode_header_text(&subject),
        from: parse_address(&from_value),
        to: parse_addresses(&to_value),
        date,
        snippet: String::new(),
    })
}

pub fn parse_full(raw: &[u8]) -> Result<ParsedMessage> {
    let parsed = parse_mail(raw)?;
    let (headers, _) = parse_headers(raw)?;

    let subject = headers
        .get_first_value("Subject")
        .map(|value| decode_header_text(&value))
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

fn find_part<'a>(mail: &'a ParsedMail<'a>, mime: &str) -> Option<&'a ParsedMail<'a>> {
    if mail.ctype.mimetype.eq_ignore_ascii_case(mime) {
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
            out.push(Attachment {
                filename,
                mime: mail.ctype.mimetype.clone(),
                size: mail.get_body_raw().map(|body| body.len()).unwrap_or(0),
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

fn decode_header_text(value: &str) -> String {
    mailparse::parse_header(value.as_bytes())
        .map(|(header, _)| header.get_value())
        .unwrap_or_else(|_| value.to_string())
}

fn make_snippet(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut snippet = collapsed.chars().take(180).collect::<String>();
    if collapsed.chars().count() > 180 {
        snippet.push('…');
    }
    snippet
}
