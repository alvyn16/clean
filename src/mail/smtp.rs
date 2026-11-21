use anyhow::{anyhow, Context, Result};
use lettre::message::{header::ContentType, Attachment as MailAttachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::{Message, SmtpTransport, Transport};
use std::time::Duration;

use crate::config::{AccountConfig, TlsMode};
use crate::mail::{Draft, MailAddress};

fn mailbox(address: &MailAddress) -> Result<Mailbox> {
    let email = address
        .email
        .parse()
        .with_context(|| format!("invalid email address {:?}", address.email))?;
    let name = if address.name.trim().is_empty() {
        None
    } else {
        Some(address.name.clone())
    };
    Ok(Mailbox::new(name, email))
}

fn build_message(from: &MailAddress, draft: &Draft) -> Result<Message> {
    if draft.to.is_empty() {
        return Err(anyhow!("add at least one recipient"));
    }

    let mut builder = Message::builder()
        .from(mailbox(from)?)
        .subject(draft.subject.clone())
        .date_now()
        .message_id(draft.message_id.clone().map(|id| {
            if id.starts_with('<') {
                id
            } else {
                format!("<{id}>")
            }
        }));

    for recipient in &draft.to {
        builder = builder.to(mailbox(recipient)?);
    }
    for recipient in &draft.cc {
        builder = builder.cc(mailbox(recipient)?);
    }
    for recipient in &draft.bcc {
        builder = builder.bcc(mailbox(recipient)?);
    }
    if let Some(parent) = &draft.in_reply_to {
        builder = builder.in_reply_to(parent.clone());
    }
    for reference in &draft.references {
        builder = builder.references(reference.clone());
    }

    let text = SinglePart::builder()
        .header(ContentType::TEXT_PLAIN)
        .body(draft.body_text.clone());

    let body = if draft.body_html.as_deref().is_some_and(|html| !html.is_empty()) {
        MultiPart::alternative_plain_html(
            draft.body_text.clone(),
            draft.body_html.clone().unwrap_or_default(),
        )
    } else {
        MultiPart::mixed().singlepart(text)
    };

    // Wrap the body in a mixed part and add one part per attachment so the
    // same bytes are sent, saved to Sent, and uploaded as a draft.
    let mut mixed = MultiPart::mixed().multipart(body);
    for attachment in &draft.attachments {
        let bytes = std::fs::read(&attachment.path).with_context(|| {
            format!("reading attachment {}", attachment.path.display())
        })?;
        let content_type = ContentType::parse(&attachment.mime)
            .unwrap_or_else(|_| ContentType::parse("application/octet-stream").expect("valid MIME"));
        mixed = mixed.singlepart(
            MailAttachment::new(attachment.filename.clone()).body(bytes, content_type),
        );
    }

    Ok(builder.multipart(mixed)?)
}

/// Build the raw RFC822 message bytes for a draft. Used by `send` and by the
/// sent-folder append so the same bytes are delivered and archived.
pub fn build_message_raw(from: &MailAddress, draft: &Draft) -> Result<Vec<u8>> {
    let message = build_message(from, draft)?;
    Ok(message.formatted().to_vec())
}

/// Send through the account's SMTP server. `secret` is a password or an OAuth
/// access token, depending on the provider.
pub fn send(account: &AccountConfig, secret: &str, draft: &Draft) -> Result<()> {
    let from = if draft.from.email.is_empty() {
        MailAddress {
            name: account.display_name.clone(),
            email: account.email.clone(),
        }
    } else {
        draft.from.clone()
    };

    let message = build_message(&from, draft)?;

    let mut builder = match account.smtp_tls {
        TlsMode::Implicit => SmtpTransport::relay(&account.smtp_host)?.port(account.smtp_port),
        TlsMode::Starttls => {
            SmtpTransport::starttls_relay(&account.smtp_host)?.port(account.smtp_port)
        }
        TlsMode::None => SmtpTransport::builder_dangerous(&account.smtp_host).port(account.smtp_port),
    };

    let credentials = Credentials::new(account.username.clone(), secret.to_string());
    builder = builder.credentials(credentials).timeout(Some(Duration::from_secs(30)));

    if account.provider == crate::config::Provider::Google {
        builder = builder.authentication(vec![Mechanism::Xoauth2]);
    }

    let transport = builder.build();

    transport
        .send(&message)
        .map(|_response| ())
        .with_context(|| format!("sending through {}:{}", account.smtp_host, account.smtp_port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_plain_message() {
        let from = MailAddress {
            name: "Alvyn".into(),
            email: "me@example.com".into(),
        };
        let draft = Draft {
            from: from.clone(),
            to: vec![MailAddress {
                name: "Dog Park".into(),
                email: "crew@example.com".into(),
            }],
            subject: "Saturday".into(),
            body_text: "Canyon loop at 8.".into(),
            ..Default::default()
        };
        let message = build_message(&from, &draft).unwrap();
        let raw = String::from_utf8_lossy(&message.formatted()).to_string();
        assert!(raw.contains("Subject: Saturday"));
        assert!(raw.contains("crew@example.com"));
    }

}


#[cfg(test)]
mod attachment_tests {
    use super::*;

    #[test]
    fn builds_message_with_attachment() {
        let dir = std::env::temp_dir().join(format!("clean-attach-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hello.txt");
        std::fs::write(&path, b"hello attachment").unwrap();

        let from = MailAddress { name: "Me".into(), email: "me@example.com".into() };
        let draft = Draft {
            from: from.clone(),
            to: vec![MailAddress { name: "You".into(), email: "you@example.com".into() }],
            subject: "with file".into(),
            body_text: "body".into(),
            attachments: vec![crate::mail::OutgoingAttachment {
                filename: "hello.txt".into(),
                mime: "text/plain".into(),
                size: 16,
                path: path.clone(),
            }],
            ..Default::default()
        };
        let raw = String::from_utf8_lossy(&build_message_raw(&from, &draft).unwrap()).into_owned();
        assert!(raw.contains("filename=\"hello.txt\"") || raw.contains("name=\"hello.txt\""));
        assert!(raw.contains("hello attachment"), "attachment bytes missing: {raw}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    #[ignore]
    fn probe_send_attachment_mail() {
        let config = crate::config::Config::load().unwrap();
        let account = config.accounts.first().unwrap().clone();
        let sender = config.accounts.get(1).cloned().unwrap_or_else(|| account.clone());
        let credential = crate::mail::auth::credential(&sender, &config).unwrap();
        let secret = match &credential {
            crate::mail::imap::Credential::Password(v) => v.clone(),
            crate::mail::imap::Credential::OAuth(v) => v.clone(),
        };
        let id = format!("clean-attachment-probe-{}@clean.local", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let path = std::env::temp_dir().join("clean-attachment-probe.txt");
        std::fs::write(&path, b"attachment probe contents").unwrap();
        let from = MailAddress { name: sender.display_name.clone(), email: sender.email.clone() };
        let draft = Draft {
            from: from.clone(),
            to: vec![MailAddress { name: account.display_name.clone(), email: account.email.clone() }],
            subject: "attachment probe".into(),
            body_text: "This message has a text attachment.".into(),
            message_id: Some(id.clone()),
            attachments: vec![crate::mail::OutgoingAttachment {
                filename: "probe-note.txt".into(),
                mime: "text/plain".into(),
                size: 25,
                path: path.clone(),
            }],
            ..Default::default()
        };
        crate::mail::smtp::send(&sender, &secret, &draft).expect("send attachment probe");
        println!("sent attachment probe id={id}");
        std::thread::sleep(std::time::Duration::from_secs(25));
        let receiver_credential = crate::mail::auth::credential(&account, &config).unwrap();
        let mut session = crate::mail::imap::connect(&account, &receiver_credential).unwrap();
        let removed = session.delete_by_message_id("INBOX", &id).unwrap();
        session.logout();
        println!("removed probe: {removed}");
        assert_eq!(removed, 1);
    }

}
