use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::theme::{Accent, ThemeMode};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub appearance: Appearance,
    pub window: WindowConfig,
    pub onboarding: Onboarding,
    pub google: GoogleConfig,
    pub calendar: CalendarConfig,
    pub accounts: Vec<AccountConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            appearance: Appearance::default(),
            window: WindowConfig::default(),
            onboarding: Onboarding::default(),
            google: GoogleConfig::default(),
            calendar: CalendarConfig::default(),
            accounts: Vec::new(),
        }
    }
}

/// Google sign-in uses a Desktop OAuth client. The client ID is public; the
/// client secret is kept in the keyring.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GoogleConfig {
    pub client_id: Option<String>,
}

impl Default for GoogleConfig {
    fn default() -> Self {
        Self {
            client_id: Self::embedded_client_id().map(str::to_string),
        }
    }
}

impl GoogleConfig {
    /// An OAuth client baked into the binary at build time via
    /// `CLEAN_GOOGLE_CLIENT_ID`. Desktop clients are public, so this is safe to embed.
    pub fn embedded_client_id() -> Option<&'static str> {
        match option_env!("CLEAN_GOOGLE_CLIENT_ID") {
            Some(value) if !value.is_empty() => Some(value),
            _ => None,
        }
    }

    /// Matching client secret baked in at build time via `CLEAN_GOOGLE_CLIENT_SECRET`.
    pub fn embedded_client_secret() -> Option<&'static str> {
        match option_env!("CLEAN_GOOGLE_CLIENT_SECRET") {
            Some(value) if !value.is_empty() => Some(value),
            _ => None,
        }
    }

    pub fn has_embedded_client(&self) -> bool {
        Self::embedded_client_id().is_some() && Self::embedded_client_secret().is_some()
    }

    pub fn is_configured(&self) -> bool {
        self.client_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    }
}

/// Settings for an optional CalDAV calendar. The password is kept in the
/// keyring; only the URL and username live in config.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CalendarConfig {
    pub url: String,
    pub username: String,
    pub sync_interval_secs: u64,
}

impl Default for CalendarConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            username: String::new(),
            sync_interval_secs: 300,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub mode: ThemeMode,
    pub accent: String,
    pub text_scale: f32,
    pub reduce_motion: bool,
    #[serde(default = "default_load_remote_images")]
    pub load_remote_images: bool,
    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Omarchy,
            accent: "blue".into(),
            text_scale: 1.0,
            reduce_motion: false,
            load_remote_images: true,
            sync_interval_secs: 10,
        }
    }
}

impl Appearance {
    pub fn accent(&self) -> Accent {
        Accent::from_config_str(&self.accent)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub width: f32,
    pub height: f32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1440.0,
            height: 900.0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Onboarding {
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountConfig {
    pub id: String,
    pub provider: Provider,
    pub email: String,
    pub display_name: String,
    /// Google profile photo URL (may be a static image or an animated GIF).
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// IMAP/SMTP login. Usually the email, but Proton Bridge uses its own username.
    pub username: String,
    pub imap_host: String,
    pub imap_port: u16,
    #[serde(default = "default_imap_tls")]
    pub imap_tls: TlsMode,
    pub smtp_host: String,
    pub smtp_port: u16,
    #[serde(default = "default_smtp_tls")]
    pub smtp_tls: TlsMode,
}

fn default_load_remote_images() -> bool {
    true
}

fn default_sync_interval() -> u64 {
    10
}

fn default_imap_tls() -> TlsMode {
    TlsMode::Implicit
}

fn default_smtp_tls() -> TlsMode {
    TlsMode::Starttls
}

impl AccountConfig {
    pub fn for_provider(provider: Provider, email: &str, display_name: &str) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let (imap_host, imap_port, smtp_host, smtp_port, imap_tls, smtp_tls) = match provider {
            Provider::Google => (
                "imap.gmail.com",
                993,
                "smtp.gmail.com",
                587,
                TlsMode::Implicit,
                TlsMode::Starttls,
            ),
            Provider::Apple => (
                "imap.mail.me.com",
                993,
                "smtp.mail.me.com",
                587,
                TlsMode::Implicit,
                TlsMode::Starttls,
            ),
            Provider::Proton => (
                "127.0.0.1",
                1143,
                "127.0.0.1",
                1025,
                TlsMode::Starttls,
                TlsMode::Starttls,
            ),
        };

        Self {
            id,
            provider,
            email: email.to_string(),
            display_name: display_name.to_string(),
            avatar_url: None,
            username: email.to_string(),
            imap_host: imap_host.into(),
            imap_port,
            imap_tls,
            smtp_host: smtp_host.into(),
            smtp_port,
            smtp_tls,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Google,
    Apple,
    Proton,
}

impl Provider {
    pub fn label(self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::Apple => "Apple",
            Self::Proton => "Proton",
        }
    }

    pub fn help_url(self) -> &'static str {
        match self {
            Self::Google => "https://console.cloud.google.com/apis/credentials",
            Self::Apple => "https://appleid.apple.com/account/manage",
            Self::Proton => "https://proton.me/mail/bridge",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TlsMode {
    #[default]
    Implicit,
    Starttls,
    None,
}

impl Config {
    pub fn path() -> PathBuf {
        config_dir().join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let mut config: Self =
            toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
        // Signing in again used to append a second entry for the same mailbox.
        let mut seen = std::collections::HashSet::new();
        config
            .accounts
            .retain(|account| seen.insert(account.email.trim().to_ascii_lowercase()));
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let raw = toml::to_string_pretty(self)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, raw).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("installing {}", path.display()))?;
        Ok(())
    }

    pub fn account(&self, needle: &str) -> Option<&AccountConfig> {
        let needle = needle.trim().to_ascii_lowercase();
        self.accounts.iter().find(|account| {
            account.id.eq_ignore_ascii_case(&needle)
                || account.email.eq_ignore_ascii_case(&needle)
        })
    }

    pub fn upsert_account(&mut self, account: AccountConfig) {
        let email = account.email.trim().to_ascii_lowercase();
        if let Some(existing) = self.accounts.iter_mut().find(|candidate| {
            candidate.id == account.id || candidate.email.trim().to_ascii_lowercase() == email
        }) {
            // Keep the existing id so cached mail and avatars stay attached.
            let id = existing.id.clone();
            *existing = AccountConfig { id, ..account };
        } else {
            self.accounts.push(account);
        }
    }

    pub fn remove_account(&mut self, id: &str) {
        self.accounts.retain(|account| account.id != id);
    }
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
        .join("clean")
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share"))
        .join("clean")
}

pub fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"))
        .join("clean")
}
