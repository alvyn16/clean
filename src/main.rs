// Several helpers are intentionally kept for planned milestones (drafts,
// attachments, event deletion, alternate TLS paths). Silence the dead-code
// warnings so release builds are easy to read.
#![allow(dead_code)]

mod cli;
mod config;
mod db;
mod mail;
mod secrets;
mod theme;
mod ui;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("clean=info")),
        )
        .init();

    let cli = cli::Cli::parse();
    match cli.command {
        Some(command) => cli::run(command),
        None => {
            ui::run()
        }
    }
}
