use anyhow::{anyhow, Context, Result};
use lettre::message::{header::ContentType, Mailbox, MultiPart, SinglePart};
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
        .date_now();

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

    let message = if draft.body_html.as_deref().is_some_and(|html| !html.is_empty()) {
        builder.multipart(MultiPart::alternative_plain_html(
            draft.body_text.clone(),
            draft.body_html.clone().unwrap_or_default(),
        ))?
    } else {
        builder.singlepart(text)?
    };

    Ok(message)
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
