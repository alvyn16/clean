use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::config::{AccountConfig, Config, GoogleConfig, Provider};
use crate::mail::imap::{self, Credential};
use crate::mail::oauth;
use crate::secrets;

pub fn credential(account: &AccountConfig, config: &Config) -> Result<Credential> {
    match account.provider {
        Provider::Google => {
            let client_id = config
                .google
                .client_id
                .clone()
                .ok_or_else(|| anyhow!("Google OAuth is not configured yet; run `clean auth google`"))?;
            let client_secret = oauth::client_secret()?
                .ok_or_else(|| anyhow!("the Google client secret is missing; run `clean auth google`"))?;
            let refresh = secrets::get(&account.id, "oauth_refresh")?
                .ok_or_else(|| anyhow!("this Google account needs to sign in again"))?;
            let token = oauth::access_token(&account.id, &client_id, &client_secret, &refresh)?;
            Ok(Credential::OAuth(token))
        }
        Provider::Apple | Provider::Proton => {
            let password = secrets::get(&account.id, "password")?.ok_or_else(|| {
                anyhow!(
                    "no password stored for {}; reconnect the account",
                    account.email
                )
            })?;
            Ok(Credential::Password(password))
        }
    }
}

pub fn connect_google(
    config: &mut Config,
    client_id: Option<String>,
    client_secret: Option<String>,
    email_hint: Option<String>,
) -> Result<AccountConfig> {
    let client_id = resolve_client_id(config, client_id)?;
    let client_secret = resolve_client_secret(client_secret)?;
    let flow = oauth::begin_google_auth(&client_id)?;
    let session = flow.finish(&client_id, &client_secret, None)?;
    account_from_google_session(config, session, email_hint)
}

fn account_from_google_session(
    config: &mut Config,
    session: oauth::GoogleSession,
    email_hint: Option<String>,
) -> Result<AccountConfig> {
    let display_name = if let Some(hint) = email_hint {
        hint
    } else {
        session.email.clone()
    };
    let name = display_name
        .split('@')
        .next()
        .unwrap_or(&display_name)
        .to_string();

    let mut account = AccountConfig::for_provider(Provider::Google, &session.email, &name);
    account.avatar_url = session.picture.clone();

    secrets::set(&account.id, "oauth_refresh", &session.refresh_token)?;
    secrets::set(&account.id, "oauth_access", &session.access_token)?;
    secrets::set(
        &account.id,
        "oauth_expiry",
        &session.expires_at.to_string(),
    )?;

    let credential = Credential::OAuth(session.access_token);
    verify(&account, &credential)?;

    config.upsert_account(account.clone());
    config.save()?;
    Ok(account)
}

/// Complete a Google connection from a flow whose browser was already opened.
pub fn connect_google_with_flow(
    config: &mut Config,
    flow: oauth::GoogleAuthFlow,
    client_id: &str,
    client_secret: &str,
    email_hint: Option<String>,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<AccountConfig> {
    secrets::set(secrets::APP, oauth::CLIENT_SECRET_KIND, client_secret)?;
    config.google.client_id = Some(client_id.to_string());
    config.save()?;

    let session = flow.finish(client_id, client_secret, cancel)?;
    account_from_google_session(config, session, email_hint)
}

pub fn connect_apple(config: &mut Config, email: &str) -> Result<AccountConfig> {
    let email = email.trim();
    if email.is_empty() || !email.contains('@') {
        return Err(anyhow!("enter your full iCloud email address"));
    }

    println!(
        "Apple requires an app-specific password.\n\
         Create one at https://account.apple.com → Sign-In and Security → App-Specific Passwords."
    );
    let password = prompt_secret("App-specific password: ")?;
    connect_apple_with_password(config, email, &password)
}

pub fn connect_apple_with_password(
    config: &mut Config,
    email: &str,
    password: &str,
) -> Result<AccountConfig> {
    let email = email.trim();
    if email.is_empty() || !email.contains('@') {
        return Err(anyhow!("enter your full iCloud email address"));
    }
    if password.trim().is_empty() {
        return Err(anyhow!("enter the app-specific password"));
    }

    let account = AccountConfig::for_provider(Provider::Apple, email, email);
    let password = password.trim().to_string();
    secrets::set(&account.id, "password", &password)?;

    let credential = Credential::Password(password);
    verify(&account, &credential)?;

    config.upsert_account(account.clone());
    config.save()?;
    Ok(account)
}

pub fn connect_proton(config: &mut Config, email: &str) -> Result<AccountConfig> {
    let email = email.trim();
    if email.is_empty() || !email.contains('@') {
        return Err(anyhow!("enter your Proton email address"));
    }

    println!(
        "Proton needs Proton Mail Bridge running locally.\n\
         Open Bridge → your account → Mailbox details for the IMAP/SMTP password."
    );
    let password = prompt_secret("Bridge password: ")?;
    connect_proton_with_password(config, email, &password)
}

pub fn connect_proton_with_password(
    config: &mut Config,
    email: &str,
    password: &str,
) -> Result<AccountConfig> {
    let email = email.trim();
    if email.is_empty() || !email.contains('@') {
        return Err(anyhow!("enter your Proton email address"));
    }
    if password.trim().is_empty() {
        return Err(anyhow!("enter the Proton Bridge password"));
    }

    let account = AccountConfig::for_provider(Provider::Proton, email, email);
    let password = password.trim().to_string();
    secrets::set(&account.id, "password", &password)?;

    let credential = Credential::Password(password);
    verify(&account, &credential)?;

    config.upsert_account(account.clone());
    config.save()?;
    Ok(account)
}

pub fn disconnect(config: &mut Config, needle: &str) -> Result<AccountConfig> {
    let account = config
        .account(needle)
        .cloned()
        .ok_or_else(|| anyhow!("no account matches {needle:?}"))?;

    config.remove_account(&account.id);
    config.save()?;

    if let Ok(db) = crate::db::Db::open_default() {
        let _ = db.clear_account(&account.id);
    }
    let _ = secrets::delete(&account.id, "password");
    oauth::forget(&account.id);

    Ok(account)
}

fn resolve_client_id(config: &Config, provided: Option<String>) -> Result<String> {
    if let Some(value) = provided.or_else(|| std::env::var("CLEAN_GOOGLE_CLIENT_ID").ok())
        && !value.trim().is_empty() {
            return Ok(value.trim().to_string());
        }
    if config.google.is_configured() && GoogleConfig::embedded_client_id().is_none() {
        return Ok(config.google.client_id.clone().unwrap_or_default());
    }
    if let Some(default) = GoogleConfig::embedded_client_id() {
        let _ = default;
        return Ok(default.to_string());
    }

    println!(
        "Google sign-in needs an OAuth 2.0 Desktop client.\n\
         Create one at https://console.cloud.google.com/apis/credentials\n\
         (enable the Gmail API, then create an OAuth client of type \"Desktop app\")."
    );
    print!("Client ID: ");
    std::io::stdout().flush().ok();
    let mut value = String::new();
    std::io::stdin().read_line(&mut value)?;
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(anyhow!("a Google client ID is required"));
    }
    Ok(value)
}

fn resolve_client_secret(provided: Option<String>) -> Result<String> {
    if let Some(value) = provided.or_else(|| std::env::var("CLEAN_GOOGLE_CLIENT_SECRET").ok())
        && !value.trim().is_empty() {
            return Ok(value.trim().to_string());
        }
    if let Some(default) = GoogleConfig::embedded_client_secret() {
        return Ok(default.to_string());
    }
    if let Some(value) = oauth::client_secret()?
        && !value.trim().is_empty() {
            return Ok(value);
        }

    print!("Client secret: ");
    std::io::stdout().flush().ok();
    let value = prompt_secret("")?;
    if value.is_empty() {
        return Err(anyhow!("a Google client secret is required"));
    }
    Ok(value)
}

fn prompt_secret(prompt: &str) -> Result<String> {
    let value = rpassword::prompt_password(prompt)?;
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(anyhow!("a password is required"));
    }
    Ok(value)
}

/// Connect and list folders, proving the credentials work.
pub fn verify(account: &AccountConfig, credential: &Credential) -> Result<usize> {
    let mut session = imap::connect(account, credential)?;
    let folders = session.list()?;
    session.logout();
    Ok(folders.len())
}
