use std::io::{Read, Write};
use std::net::TcpStream;

use anyhow::{anyhow, Context, Result};
use native_tls::TlsConnector;

use crate::config::{AccountConfig, TlsMode};

#[derive(Clone)]
pub enum Credential {
    Password(String),
    OAuth(String),
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Password(_) => f.write_str("Credential::Password(…)"),
            Self::OAuth(_) => f.write_str("Credential::OAuth(…)"),
        }
    }
}

struct XOAuth2 {
    user: String,
    token: String,
}

impl imap::Authenticator for XOAuth2 {
    type Response = String;

    fn process(&self, _challenge: &[u8]) -> Self::Response {
        // https://developers.google.com/gmail/imap/xoauth2-protocol
        format!("user={}\x01auth=Bearer {}\x01\x01", self.user, self.token)
    }
}

#[derive(Debug, Clone)]
pub struct RemoteFolder {
    pub name: String,
    pub delimiter: Option<String>,
    pub selectable: bool,
}

#[derive(Debug, Clone)]
pub struct MailboxInfo {
    pub exists: u32,
    pub unseen: u32,
    pub uid_validity: u32,
    pub uid_next: u32,
}

#[derive(Debug, Clone)]
pub struct FetchedMessage {
    pub uid: u32,
    pub flags: String,
    pub raw: Vec<u8>,
}

pub enum Session {
    Tls(imap::Session<native_tls::TlsStream<TcpStream>>),
    Plain(imap::Session<TcpStream>),
}

pub fn connect(account: &AccountConfig, credential: &Credential) -> Result<Session> {
    let tls = TlsConnector::builder()
        .build()
        .context("preparing TLS for the mail server")?;

    match account.imap_tls {
        TlsMode::Implicit => {
            let client = imap::connect(
                (account.imap_host.as_str(), account.imap_port),
                &account.imap_host,
                &tls,
            )
            .with_context(|| format!("connecting to {}:{}", account.imap_host, account.imap_port))?;
            let session = authenticate(client, account, credential)?;
            Ok(Session::Tls(session))
        }
        TlsMode::Starttls => {
            let client = imap::connect_starttls(
                (account.imap_host.as_str(), account.imap_port),
                &account.imap_host,
                &tls,
            )
            .with_context(|| {
                format!(
                    "starting TLS with {}:{}",
                    account.imap_host, account.imap_port
                )
            })?;
            let session = authenticate(client, account, credential)?;
            Ok(Session::Tls(session))
        }
        TlsMode::None => {
            let stream = TcpStream::connect((account.imap_host.as_str(), account.imap_port))
                .with_context(|| {
                    format!("connecting to {}:{}", account.imap_host, account.imap_port)
                })?;
            stream.set_nodelay(true).ok();
            let client = imap::Client::new(stream);
            let session = authenticate(client, account, credential)?;
            Ok(Session::Plain(session))
        }
    }
}

fn authenticate<T: Read + Write>(
    client: imap::Client<T>,
    account: &AccountConfig,
    credential: &Credential,
) -> Result<imap::Session<T>> {
    match credential {
        Credential::Password(password) => client
            .login(&account.username, password)
            .map_err(|(error, _)| anyhow!("signing in to {}: {error}", account.imap_host)),
        Credential::OAuth(token) => {
            let auth = XOAuth2 {
                user: account.username.clone(),
                token: token.clone(),
            };
            client
                .authenticate("XOAUTH2", &auth)
                .map_err(|(error, _)| anyhow!("signing in to {}: {error}", account.imap_host))
        }
    }
}

impl Session {
    pub fn list(&mut self) -> Result<Vec<RemoteFolder>> {
        match self {
            Self::Tls(session) => list(session),
            Self::Plain(session) => list(session),
        }
    }

    pub fn select(&mut self, folder: &str) -> Result<MailboxInfo> {
        match self {
            Self::Tls(session) => select(session, folder),
            Self::Plain(session) => select(session, folder),
        }
    }

    /// Total and unread counts without changing the selected mailbox.
    pub fn status(&mut self, folder: &str) -> Result<(u32, u32)> {
        match self {
            Self::Tls(session) => status(session, folder),
            Self::Plain(session) => status(session, folder),
        }
    }

    pub fn headers(&mut self, folder: &str, first: u32, last: u32) -> Result<Vec<FetchedMessage>> {
        match self {
            Self::Tls(session) => headers(session, folder, first, last),
            Self::Plain(session) => headers(session, folder, first, last),
        }
    }

    pub fn fetch_body(&mut self, folder: &str, uid: u32) -> Result<Vec<u8>> {
        match self {
            Self::Tls(session) => fetch_body(session, folder, uid),
            Self::Plain(session) => fetch_body(session, folder, uid),
        }
    }

    pub fn mark_seen(&mut self, folder: &str, uid: u32) -> Result<()> {
        match self {
            Self::Tls(session) => mark_seen(session, folder, uid),
            Self::Plain(session) => mark_seen(session, folder, uid),
        }
    }

    pub fn append(&mut self, folder: &str, raw: &[u8]) -> Result<()> {
        match self {
            Self::Tls(session) => session
                .append_with_flags(folder, raw, &[imap::types::Flag::Seen])
                .map_err(Into::into),
            Self::Plain(session) => session
                .append_with_flags(folder, raw, &[imap::types::Flag::Seen])
                .map_err(Into::into),
        }
    }

    pub fn logout(mut self) {
        match &mut self {
            Self::Tls(session) => {
                let _ = session.logout();
            }
            Self::Plain(session) => {
                let _ = session.logout();
            }
        }
    }
}

fn list<T: Read + Write>(session: &mut imap::Session<T>) -> Result<Vec<RemoteFolder>> {
    let names = session.list(Some(""), Some("*"))?;
    Ok(names
        .iter()
        .filter(|name| !name.name().eq_ignore_ascii_case("[gmail]"))
        .map(|name| RemoteFolder {
            name: name.name().to_string(),
            delimiter: name.delimiter().map(str::to_string),
            selectable: !name
                .attributes()
                .iter()
                .any(|attribute| matches!(attribute, imap::types::NameAttribute::NoSelect)),
        })
        .collect())
}

fn select<T: Read + Write>(session: &mut imap::Session<T>, folder: &str) -> Result<MailboxInfo> {
    let mailbox = session.select(folder)?;
    Ok(MailboxInfo {
        exists: mailbox.exists,
        unseen: mailbox.unseen.unwrap_or(0),
        uid_validity: mailbox.uid_validity.unwrap_or(0),
        uid_next: mailbox.uid_next.unwrap_or(0),
    })
}

fn status<T: Read + Write>(
    session: &mut imap::Session<T>,
    folder: &str,
) -> Result<(u32, u32)> {
    let mailbox = session.status(folder, "(MESSAGES UNSEEN)")?;
    Ok((mailbox.exists, mailbox.unseen.unwrap_or(0)))
}

fn headers<T: Read + Write>(
    session: &mut imap::Session<T>,
    folder: &str,
    first: u32,
    last: u32,
) -> Result<Vec<FetchedMessage>> {
    session.select(folder)?;
    let range = if first == 0 {
        "1:*".to_string()
    } else {
        format!("{first}:{last}")
    };
    let fetches = session.uid_fetch(range, "(UID FLAGS RFC822.HEADER)")?;
    let mut messages = Vec::with_capacity(fetches.len());
    for fetch in fetches.iter() {
        let Some(uid) = fetch.uid else { continue };
        let Some(raw) = fetch.header() else { continue };
        messages.push(FetchedMessage {
            uid,
            flags: fetch
                .flags()
                .iter()
                .map(|flag| flag.to_string())
                .collect::<Vec<_>>()
                .join(" "),
            raw: raw.to_vec(),
        });
    }
    Ok(messages)
}

fn fetch_body<T: Read + Write>(
    session: &mut imap::Session<T>,
    folder: &str,
    uid: u32,
) -> Result<Vec<u8>> {
    session.select(folder)?;
    let fetches = session.uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")?;
    let raw = fetches
        .iter()
        .find_map(|fetch| fetch.body())
        .ok_or_else(|| anyhow!("message {uid} had no body"))?;
    Ok(raw.to_vec())
}

fn mark_seen<T: Read + Write>(
    session: &mut imap::Session<T>,
    folder: &str,
    uid: u32,
) -> Result<()> {
    session.select(folder)?;
    session.uid_store(uid.to_string(), "+FLAGS (\\Seen)")?;
    Ok(())
}
