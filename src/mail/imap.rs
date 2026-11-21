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

/// Result of waiting on an IMAP IDLE subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleOutcome {
    /// The selected mailbox changed on the server.
    Changed,
    /// The timeout elapsed; the caller should re-issue IDLE.
    TimedOut,
}

/// Seconds to wait for one address before moving on to the next.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

/// Resolve a server, preferring IPv4. On networks with a broken IPv6 route a
/// plain `TcpStream::connect` would otherwise stall for minutes before failing.
fn server_addrs(host: &str, port: u16) -> Result<Vec<std::net::SocketAddr>> {
    use std::net::ToSocketAddrs;
    let mut addrs: Vec<std::net::SocketAddr> = (host, port)
        .to_socket_addrs()
        .with_context(|| format!("resolving {host}"))?
        .collect();
    if addrs.is_empty() {
        return Err(anyhow!("{host} did not resolve to any address"));
    }
    addrs.sort_by_key(|addr| u8::from(addr.is_ipv6()));
    Ok(addrs)
}

fn tcp_stream(addr: std::net::SocketAddr) -> Result<TcpStream> {
    let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)
        .with_context(|| format!("connecting to {addr}"))?;
    stream.set_nodelay(true).ok();
    Ok(stream)
}

pub fn connect(account: &AccountConfig, credential: &Credential) -> Result<Session> {
    let tls = TlsConnector::builder()
        .build()
        .context("preparing TLS for the mail server")?;
    let addrs = server_addrs(&account.imap_host, account.imap_port)?;
    let mut last_error: Option<anyhow::Error> = None;

    for addr in addrs {
        let attempt = (|| -> Result<Session> {
            match account.imap_tls {
                TlsMode::Implicit => {
                    let stream = tcp_stream(addr)?;
                    let stream = tls
                        .connect(&account.imap_host, stream)
                        .with_context(|| format!("TLS with {}", account.imap_host))?;
                    let client = imap::Client::new(stream);
                    let session = authenticate(client, account, credential)?;
                    Ok(Session::Tls(session))
                }
                TlsMode::Starttls => {
                    let client =
                        imap::connect_starttls(addr, &account.imap_host, &tls).with_context(|| {
                            format!(
                                "starting TLS with {}:{}",
                                account.imap_host, account.imap_port
                            )
                        })?;
                    let session = authenticate(client, account, credential)?;
                    Ok(Session::Tls(session))
                }
                TlsMode::None => {
                    let stream = tcp_stream(addr)?;
                    let client = imap::Client::new(stream);
                    let session = authenticate(client, account, credential)?;
                    Ok(Session::Plain(session))
                }
            }
        })();
        match attempt {
            Ok(session) => return Ok(session),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        anyhow!(
            "could not reach {}:{}",
            account.imap_host,
            account.imap_port
        )
    }))
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

    /// Block until the selected mailbox changes, or `timeout` elapses. The
    /// IDLE command is terminated cleanly on both paths so the session can be
    /// reused for the next wait.
    pub fn idle_wait(&mut self, timeout: std::time::Duration) -> Result<IdleOutcome> {
        use imap::extensions::idle::WaitOutcome;
        let outcome = match self {
            Self::Tls(session) => session.idle()?.wait_with_timeout(timeout)?,
            Self::Plain(session) => session.idle()?.wait_with_timeout(timeout)?,
        };
        Ok(match outcome {
            WaitOutcome::MailboxChanged => IdleOutcome::Changed,
            WaitOutcome::TimedOut => IdleOutcome::TimedOut,
        })
    }

    /// Total and unread counts without changing the selected mailbox.
    pub fn status(&mut self, folder: &str) -> Result<(u32, u32)> {
        match self {
            Self::Tls(session) => status(session, folder),
            Self::Plain(session) => status(session, folder),
        }
    }

    /// Number of unread messages in a folder (Gmail does not report a count in
    /// SELECT, so this asks the server directly).
    pub fn unseen_count(&mut self, folder: &str) -> Result<u32> {
        match self {
            Self::Tls(session) => unseen_count(session, folder),
            Self::Plain(session) => unseen_count(session, folder),
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

    /// Append a message to a folder, leaving the `\Seen` flag unset. Used for
    /// the Sent copy and for drafts.
    pub fn append_raw(&mut self, folder: &str, raw: &[u8], flags: &[imap::types::Flag]) -> Result<()> {
        match self {
            Self::Tls(session) => session
                .append_with_flags(folder, raw, flags)
                .map_err(Into::into),
            Self::Plain(session) => session
                .append_with_flags(folder, raw, flags)
                .map_err(Into::into),
        }
    }

    /// Delete every message in `folder` whose `Message-ID` header matches.
    /// Returns the number of messages removed.
    pub fn delete_by_message_id(&mut self, folder: &str, message_id: &str) -> Result<usize> {
        match self {
            Self::Tls(session) => delete_by_message_id(session, folder, message_id),
            Self::Plain(session) => delete_by_message_id(session, folder, message_id),
        }
    }

    /// Apply a raw UID STORE query, e.g. "+FLAGS (\\Seen)" or
    /// "-FLAGS (\\Seen)". Selects the folder first.
    pub fn uid_store(&mut self, folder: &str, uid: u32, query: &str) -> Result<()> {
        match self {
            Self::Tls(session) => store(session, folder, uid, query),
            Self::Plain(session) => store(session, folder, uid, query),
        }
    }

    pub fn mark_unseen(&mut self, folder: &str, uid: u32) -> Result<()> {
        self.uid_store(folder, uid, "-FLAGS (\\Seen)")
    }

    /// Find a folder on the server matching a `FolderRole` by name.
    pub fn folder_for_role(&mut self, role: crate::mail::FolderRole) -> Result<Option<String>> {
        let folders = self.list()?;
        Ok(folders
            .into_iter()
            .find(|f| f.selectable && crate::mail::FolderRole::from_folder_name(&f.name) == role)
            .map(|f| f.name))
    }

    /// Flags currently set on a message (for verification after an action).
    pub fn flags_for(&mut self, folder: &str, uid: u32) -> Result<String> {
        match self {
            Self::Tls(session) => flags_for(session, folder, uid),
            Self::Plain(session) => flags_for(session, folder, uid),
        }
    }

    pub fn mark_flagged(&mut self, folder: &str, uid: u32, flagged: bool) -> Result<()> {
        let query = if flagged {
            "+FLAGS (\\Flagged)"
        } else {
            "-FLAGS (\\Flagged)"
        };
        self.uid_store(folder, uid, query)
    }

    /// Copy a message (by UID) into another folder.
    pub fn copy_to(&mut self, folder: &str, uid: u32, dest: &str) -> Result<()> {
        match self {
            Self::Tls(session) => copy_to(session, folder, uid, dest),
            Self::Plain(session) => copy_to(session, folder, uid, dest),
        }
    }

    /// Move a message (by UID) into another folder: copy, flag deleted, expunge.
    pub fn move_to(&mut self, folder: &str, uid: u32, dest: &str) -> Result<()> {
        match self {
            Self::Tls(session) => move_to(session, folder, uid, dest),
            Self::Plain(session) => move_to(session, folder, uid, dest),
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
    let info = select(session, folder)?;
    let unread = unseen_count(session, folder).unwrap_or(0);
    Ok((info.exists, unread))
}

fn unseen_count<T: Read + Write>(
    session: &mut imap::Session<T>,
    folder: &str,
) -> Result<u32> {
    // STATUS avoids downloading a UID list the way SEARCH UNSEEN would.
    let quoted = format!("\"{}\"", folder.replace('\\', "\\\\").replace('"', "\\\""));
    if let Ok(raw) = session.run_command_and_read_response(format!("STATUS {quoted} (UNSEEN)")) {
        if let Some(count) = parse_status_value(&raw, "UNSEEN") {
            return Ok(count);
        }
    }
    session.select(folder)?;
    Ok(session.uid_search("UNSEEN")?.len() as u32)
}

fn parse_status_value(raw: &[u8], key: &str) -> Option<u32> {
    let text = String::from_utf8_lossy(raw).to_uppercase();
    let index = text.find(key)?;
    let digits: String = text[index + key.len()..]
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
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

fn flags_for<T: Read + Write>(
    session: &mut imap::Session<T>,
    folder: &str,
    uid: u32,
) -> Result<String> {
    session.select(folder)?;
    let fetches = session.uid_fetch(uid.to_string(), "(UID FLAGS)")?;
    let flags = fetches
        .iter()
        .find(|f| !f.flags().is_empty())
        .map(|f| f.flags())
        .map(|list| {
            list.iter()
                .map(|flag| format!("{flag}"))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    Ok(flags)
}

fn store<T: Read + Write>(
    session: &mut imap::Session<T>,
    folder: &str,
    uid: u32,
    query: &str,
) -> Result<()> {
    session.select(folder)?;
    session.uid_store(uid.to_string(), query)?;
    Ok(())
}

fn copy_to<T: Read + Write>(
    session: &mut imap::Session<T>,
    folder: &str,
    uid: u32,
    dest: &str,
) -> Result<()> {
    session.select(folder)?;
    session.uid_copy(uid.to_string(), dest)?;
    Ok(())
}

fn move_to<T: Read + Write>(
    session: &mut imap::Session<T>,
    folder: &str,
    uid: u32,
    dest: &str,
) -> Result<()> {
    session.select(folder)?;
    session.uid_copy(uid.to_string(), dest)?;
    session.uid_store(uid.to_string(), "+FLAGS (\\Deleted)")?;
    session.expunge()?;
    Ok(())
}

fn delete_by_message_id<T: Read + Write>(
    session: &mut imap::Session<T>,
    folder: &str,
    message_id: &str,
) -> Result<usize> {
    session.select(folder)?;
    let header = if message_id.starts_with('<') {
        message_id.to_string()
    } else {
        format!("<{message_id}>")
    };
    let query = format!("HEADER Message-ID \"{header}\"");
    let uids = session.uid_search(query)?;
    if uids.is_empty() {
        return Ok(0);
    }
    let set = uids
        .iter()
        .map(|uid| uid.to_string())
        .collect::<Vec<_>>()
        .join(",");
    session.uid_store(&set, "+FLAGS (\\Deleted)")?;
    session.expunge()?;
    Ok(uids.len())
}

#[cfg(test)]
mod draft_replace {
    use super::*;
    use crate::config::Config;

    fn append(session: &mut Session, folder: &str, account: &AccountConfig, body: &str, id: &str) {
        let from = crate::mail::MailAddress {
            name: account.display_name.clone(),
            email: account.email.clone(),
        };
        let draft = crate::mail::Draft {
            from: from.clone(),
            to: vec![crate::mail::MailAddress {
                name: "Friend".into(),
                email: "friend@example.com".into(),
            }],
            subject: "Clean replace test".into(),
            body_text: body.into(),
            message_id: Some(id.to_string()),
            ..Default::default()
        };
        let raw = crate::mail::smtp::build_message_raw(&from, &draft).expect("raw");
        // Same flow as the composer: drop the previous copy, then append.
        let _ = session.delete_by_message_id(folder, id);
        session
            .append_raw(folder, &raw, &[imap::types::Flag::Draft])
            .expect("append");
    }

    fn subject_count(session: &mut Session, folder: &str) -> usize {
        let info = session.select(folder).expect("select");
        let headers = session
            .headers(folder, 1, info.uid_next.saturating_sub(1))
            .expect("headers");
        headers
            .iter()
            .filter(|header| {
                crate::mail::parse::parse_headers_only(&header.raw)
                    .map(|summary| summary.subject == "Clean replace test")
                    .unwrap_or(false)
            })
            .count()
    }

    #[test]
    #[ignore]
    fn resaving_replaces_the_server_copy() {
        let config = Config::load().expect("config");
        let account = config.accounts[0].clone();
        let credential = crate::mail::auth::credential(&account, &config).expect("credential");
        let mut session = connect(&account, &credential).expect("connect");
        let folder = session
            .folder_for_role(crate::mail::FolderRole::Drafts)
            .expect("list")
            .expect("drafts folder");
        let id = format!(
            "draft-replace-{}@clean.local",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        append(&mut session, &folder, &account, "first version", &id);
        assert_eq!(subject_count(&mut session, &folder), 1);
        append(&mut session, &folder, &account, "second version", &id);
        assert_eq!(subject_count(&mut session, &folder), 1);

        let removed = session.delete_by_message_id(&folder, &id).expect("delete");
        assert_eq!(removed, 1);
        assert_eq!(subject_count(&mut session, &folder), 0);
        session.logout();
    }
}

#[cfg(test)]
mod idle_tests {
    use super::*;
    use crate::config::Config;

    #[test]
    #[ignore]
    fn idle_wakes_on_new_mail() {
        let config = Config::load().expect("config");
        let account = config.accounts.first().expect("an account").clone();
        let credential = crate::mail::auth::credential(&account, &config).expect("credential");

        let mut watcher = connect(&account, &credential).expect("watcher connect");
        watcher.select("INBOX").expect("select INBOX");

        // Deliver a real test message from the second configured account.
        // Gmail suppresses IDLE notifications for self-appends and flag
        // changes, but a delivered message sends a genuine EXISTS update.
        let sender = config.accounts.get(1).cloned().unwrap_or_else(|| account.clone());
        let sender_credential = crate::mail::auth::credential(&sender, &config).expect("sender auth");
        let secret = match &sender_credential {
            Credential::Password(value) => value.clone(),
            Credential::OAuth(value) => value.clone(),
        };
        let id = format!(
            "clean-idle-test-{}@clean.local",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let draft_id = id.clone();
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
            subject: "clean IDLE test".into(),
            body_text: "temporary message".into(),
            message_id: Some(id.clone()),
            ..Default::default()
        };
        let deliverer = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(1));
            crate::mail::smtp::send(&sender, &secret, &draft).expect("deliver test message");
        });

        let outcome = watcher
            .idle_wait(std::time::Duration::from_secs(30))
            .expect("IDLE");
        deliverer.join().expect("deliverer thread");
        watcher.logout();

        // Remove the test message from the watched inbox.
        let mut cleaner = connect(&account, &credential).expect("cleaner connect");
        let removed = cleaner
            .delete_by_message_id("INBOX", &draft_id)
            .expect("delete test message");
        cleaner.logout();
        println!("IDLE outcome: {outcome:?}, removed: {removed}");
        assert_eq!(outcome, IdleOutcome::Changed, "IDLE missed the new message");
        assert_eq!(removed, 1, "test message was not cleaned up");
    }

    #[test]
    #[ignore]
    fn idle_on_inbox_accepts_and_terminates_cleanly() {
        let config = Config::load().expect("config");
        let account = config.accounts.first().expect("an account");
        let credential = crate::mail::auth::credential(account, &config).expect("credential");
        let mut session = connect(account, &credential).expect("connect");
        session.select("INBOX").expect("select INBOX");
        let outcome = session
            .idle_wait(std::time::Duration::from_secs(8))
            .expect("IDLE");
        println!("IDLE outcome after 8s: {outcome:?}");
        // A timeout is expected on a quiet mailbox; either result proves the
        // IDLE command was accepted and DONE was handled without an error.
        session.logout();
    }
}
