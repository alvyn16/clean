use anyhow::{Context, Result};

const SERVICE: &str = "clean";

/// Pseudo account id for credentials that belong to the app, not one mailbox.
pub const APP: &str = "__app__";
/// Pseudo account id for the CalDAV calendar password.
pub const CALDAV: &str = "__caldav__";

fn entry(account_id: &str, kind: &str) -> Result<keyring::Entry> {
    let username = format!("{account_id}:{kind}");
    keyring::Entry::new(SERVICE, &username)
        .with_context(|| format!("opening the Secret Service entry for {username}"))
}

pub fn set(account_id: &str, kind: &str, value: &str) -> Result<()> {
    entry(account_id, kind)?
        .set_password(value)
        .with_context(|| format!("storing {kind} for account {account_id}"))?;
    Ok(())
}

pub fn get(account_id: &str, kind: &str) -> Result<Option<String>> {
    match entry(account_id, kind)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error).with_context(|| format!("reading {kind} for account {account_id}")),
    }
}

pub fn delete(account_id: &str, kind: &str) -> Result<()> {
    match entry(account_id, kind)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error).with_context(|| format!("deleting {kind} for account {account_id}")),
    }
}
