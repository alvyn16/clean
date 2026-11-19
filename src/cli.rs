use anyhow::{anyhow, Result};
use chrono::DateTime;
use clap::{Parser, Subcommand};

use crate::config::Config;
use crate::db::Db;
use crate::mail::{auth, sync};
use crate::ui;

#[derive(Parser)]
#[command(name = "clean", version, about = "Private mail for Omarchy")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Show accounts, theme, and cache stats.
    Status,
    /// Search the local mail cache.
    Search {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// List cached folders.
    Folders {
        #[arg(long)]
        account: Option<String>,
    },
    /// Sync mail from the server.
    Sync {
        #[arg(long)]
        account: Option<String>,
        /// How many recent messages to pull per folder.
        #[arg(long, default_value_t = 50)]
        limit: u32,
    },
    /// Connect or disconnect a provider.
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// Open the app.
    Ui,
    /// Open the app on the first-run setup screen.
    Setup,
}

#[derive(Subcommand)]
pub enum AuthAction {
    /// Connect Gmail with OAuth 2.0.
    Google {
        #[arg(long, env = "CLEAN_GOOGLE_CLIENT_ID")]
        client_id: Option<String>,
        #[arg(long, env = "CLEAN_GOOGLE_CLIENT_SECRET")]
        client_secret: Option<String>,
    },
    /// Connect iCloud Mail with an app-specific password.
    Apple {
        #[arg(long)]
        email: String,
    },
    /// Connect Proton Mail through Proton Bridge.
    Proton {
        #[arg(long)]
        email: String,
    },
    /// Remove an account and its cached mail.
    Disconnect { account: String },
}

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::Status => status(),
        Command::Search { query, limit } => search(&query, limit),
        Command::Folders { account } => folders(account.as_deref()),
        Command::Sync { account, limit } => sync_mail(account.as_deref(), limit),
        Command::Auth { action } => auth_provider(action),
        Command::Ui => ui::run(),
        Command::Setup => ui::run_setup(),
    }
}

fn status() -> Result<()> {
    let config = Config::load()?;
    println!("clean {}", env!("CARGO_PKG_VERSION"));
    println!(
        "theme: {} / accent: {}",
        config.appearance.mode.label(),
        config.appearance.accent
    );
    println!("config: {}", Config::path().display());
    println!("cache: {}", Db::path().display());

    if config.accounts.is_empty() {
        println!("accounts: none");
    } else {
        println!("accounts:");
        for account in &config.accounts {
            println!(
                "  {}  {}  {}",
                account.provider.label(),
                account.email,
                account.id
            );
        }
    }

    if Db::path().exists() {
        let db = Db::open_default()?;
        let stats = db.stats()?;
        println!(
            "cached: {} messages, {} unread",
            stats.messages, stats.unread
        );
    }

    Ok(())
}

fn search(query: &str, limit: u32) -> Result<()> {
    if !Db::path().exists() {
        return Err(anyhow!("no local cache yet; run `clean sync` first"));
    }
    let db = Db::open_default()?;
    let rows = db.search(query, limit)?;
    if rows.is_empty() {
        println!("no matches for {query:?}");
        return Ok(());
    }
    for row in rows {
        println!(
            "{}  {:<28}  {}",
            format_date(row.date),
            truncate(&row.from_addr, 28),
            row.subject
        );
    }
    Ok(())
}

fn folders(account: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let db = Db::open_default()?;
    let rows = match account {
        Some(needle) => {
            let id = resolve_account(&config, needle)?;
            db.folders(&id)?
        }
        None => db.all_folders()?,
    };

    if rows.is_empty() {
        println!("no cached folders yet; run `clean sync` first");
        return Ok(());
    }

    for folder in rows {
        println!(
            "{:<28} {:>6} total {:>6} unread   {}",
            folder.name, folder.total, folder.unread, folder.role
        );
    }
    Ok(())
}

fn sync_mail(account: Option<&str>, limit: u32) -> Result<()> {
    let config = Config::load()?;
    if config.accounts.is_empty() {
        return Err(anyhow!(
            "no accounts yet; run `clean auth google`, `clean auth apple`, or `clean auth proton`"
        ));
    }

    let db = Db::open_default()?;
    let accounts: Vec<_> = match account {
        Some(needle) => {
            let id = resolve_account(&config, needle)?;
            config
                .accounts
                .iter()
                .filter(|candidate| candidate.id == id)
                .cloned()
                .collect()
        }
        None => config.accounts.clone(),
    };

    for account in &accounts {
        println!("syncing {}…", account.email);
        let report = sync::sync_account(&config, &db, account, limit)?;
        println!(
            "  {} folders, {} new messages, {} failed",
            report.folders, report.fetched, report.failed
        );
    }
    Ok(())
}

fn auth_provider(action: AuthAction) -> Result<()> {
    let mut config = Config::load()?;
    match action {
        AuthAction::Google {
            client_id,
            client_secret,
        } => {
            let account = auth::connect_google(&mut config, client_id, client_secret, None)?;
            println!("connected {}", account.email);
        }
        AuthAction::Apple { email } => {
            let account = auth::connect_apple(&mut config, &email)?;
            println!("connected {}", account.email);
        }
        AuthAction::Proton { email } => {
            let account = auth::connect_proton(&mut config, &email)?;
            println!("connected {}", account.email);
        }
        AuthAction::Disconnect { account } => {
            let removed = auth::disconnect(&mut config, &account)?;
            println!("disconnected {} and cleared its cache", removed.email);
        }
    }
    Ok(())
}

fn resolve_account(config: &Config, needle: &str) -> Result<String> {
    config
        .account(needle)
        .map(|account| account.id.clone())
        .ok_or_else(|| anyhow!("no account matches {needle:?}"))
}

fn format_date(timestamp: i64) -> String {
    DateTime::from_timestamp(timestamp, 0)
        .map(|date| date.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "—".into())
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        let mut out: String = value.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
