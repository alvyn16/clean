mod calendar_rail;
mod calendar_view;
mod compose;
mod account_manager;
mod event_sheet;
mod list;
mod model;
mod reader;
mod settings;
mod setup;
mod shell;
mod sidebar;
mod topbar;
mod widgets;

use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub use model::{Field, MailView, Model, ReaderMode, Screen, SetupStep};

use gpui_kit::*;

use crate::theme::Accent;
use crate::theme::Palette;

actions!(
    clean,
    [
        Compose,
        Reply,
        ArchiveMessage,
        DeleteMessage,
        OpenSettings,
        SyncNow,
        ToggleView,
        CloseOverlay,
        NextMessage,
        PreviousMessage,
    ]
);

pub struct CleanApp {
    pub model: Model,
    pub focus: FocusHandle,
    pub hovered: Option<usize>,
    pub settings_tab: usize,
    pub notifications: [bool; 3],
    pub fetching: HashSet<i64>,
    pub search_used: bool,
    pub avatars: std::collections::HashMap<String, std::path::PathBuf>,
    pub sign_in_cancel: Option<Arc<AtomicBool>>,
}

impl CleanApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::new_with_mode(cx, false)
    }

    pub fn new_with_setup(cx: &mut Context<Self>) -> Self {
        Self::new_with_mode(cx, true)
    }

    fn new_with_mode(cx: &mut Context<Self>, force_setup: bool) -> Self {
        let mut app = Self {
            model: Model::new(force_setup),
            focus: cx.focus_handle(),
            hovered: None,
            settings_tab: 0,
            notifications: [true, true, false],
            fetching: HashSet::new(),
            search_used: false,
            avatars: Self::load_cached_avatars(),
            sign_in_cancel: None,
        };
        app.watch_omarchy_theme(cx);
        app.load_avatars(cx);
        app
    }

    /// Cached avatar path for a given account, if available.
    pub fn avatar_for(&self, account_id: &str) -> Option<std::path::PathBuf> {
        self.avatars.get(account_id).cloned()
    }

    /// Read every avatar already cached on disk, keyed by account id.
    fn load_cached_avatars() -> std::collections::HashMap<String, std::path::PathBuf> {
        let mut map = std::collections::HashMap::new();
        let config = crate::config::Config::load().unwrap_or_default();
        for account in config.accounts {
            let path = crate::config::data_dir()
                .join("avatars")
                .join(format!("{}.img", account.id));
            if path.exists() {
                map.insert(account.id, path);
            }
        }
        map
    }

    /// Download missing avatars for all accounts in the background.
    fn load_avatars(&mut self, cx: &mut Context<Self>) {
        for account in self.model.config.accounts.clone() {
            if self.avatars.contains_key(&account.id) {
                continue;
            }
            self.load_avatar(account, cx);
        }
    }

    /// Download one account's Google profile photo into the local cache.
    fn load_avatar(&mut self, account: crate::config::AccountConfig, cx: &mut Context<Self>) {
        let config = self.model.config.clone();
        if account.provider != crate::config::Provider::Google {
            return;
        }
        let client_id = crate::config::GoogleConfig::embedded_client_id()
            .map(str::to_string)
            .or_else(|| config.google.client_id.clone())
            .unwrap_or_default();
        let client_secret = crate::config::GoogleConfig::embedded_client_secret()
            .map(str::to_string)
            .or_else(|| crate::mail::oauth::client_secret().ok().flatten())
            .unwrap_or_default();

        let account_id = account.id.clone();
        let task = cx.background_executor().spawn(async move {
            let refresh = crate::secrets::get(&account.id, "oauth_refresh")
                .ok()
                .flatten()
                .unwrap_or_default();
            if refresh.is_empty() || client_id.is_empty() || client_secret.is_empty() {
                return None;
            }
            let url = match account.avatar_url.clone() {
                Some(url) => url,
                None => crate::mail::oauth::fetch_profile_picture(
                    &account.id,
                    &client_id,
                    &client_secret,
                    &refresh,
                )
                .ok()??,
            };
            crate::mail::oauth::cache_avatar(&account.id, &url).ok()
        });

        cx.spawn(async move |this, cx| {
            if let Some(path) = task.await {
                this.update(cx, |this, cx| {
                    this.avatars.insert(account_id, path);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn omarchy_fingerprint(model: &Model) -> String {        model
            .omarchy
            .as_ref()
            .map(|colors| {
                format!(
                    "{}|{}|{}",
                    colors.background().to_hex(),
                    colors.blue().to_hex(),
                    colors.mode
                )
            })
            .unwrap_or_default()
    }

    /// Follow Omarchy's live palette: re-read `colors.toml` every two seconds.
    fn watch_omarchy_theme(&mut self, cx: &mut Context<Self>) {
        let mut last = Self::omarchy_fingerprint(&self.model);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(2))
                    .await;
                let fresh = crate::theme::omarchy::read_live();
                let fingerprint = fresh
                    .as_ref()
                    .map(|colors| {
                        format!(
                            "{}|{}|{}",
                            colors.background().to_hex(),
                            colors.blue().to_hex(),
                            colors.mode
                        )
                    })
                    .unwrap_or_default();
                if fingerprint == last {
                    continue;
                }
                last = fingerprint;
                this.update(cx, |this, cx| {
                    this.model.omarchy = fresh;
                    if this.model.config.appearance.mode == crate::theme::ThemeMode::Omarchy {
                        this.model.rebuild_theme();
                        sync_component_theme(&this.model, cx);
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Show a transient message and dismiss it automatically.
    pub fn toast(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        let message = message.into();
        self.model.toast = Some((message.clone(), std::time::Instant::now()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(2600))
                .await;
            this.update(cx, |this, cx| {
                let still_current = this
                    .model
                    .toast
                    .as_ref()
                    .is_some_and(|(current, _)| current == &message);
                if still_current {
                    this.model.toast = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();
        let command = keystroke.modifiers.platform || keystroke.modifiers.control;

        if key == "escape" {
            if self.model.show_event_sheet {
                self.model.show_event_sheet = false;
                self.model.focused = None;
                cx.notify();
                return;
            }
            if self.model.show_account_manager {
                self.close_account_manager(cx);
                return;
            }
            if self.model.focused == Some(Field::Search) && self.search_used {
                self.model.search.clear();
                self.model.reload();
                self.search_used = false;
            }
            if self.model.focused.is_some() {
                self.model.focused = None;
            } else if self.model.show_settings {
                self.model.show_settings = false;
            } else if matches!(self.model.reader, ReaderMode::Reply | ReaderMode::Compose) {
                self.model.reader = ReaderMode::Read;
            }
            cx.notify();
            return;
        }

        if command {
            return;
        }

        let Some(field) = self.model.focused else {
            if key == "j" || key == "down" {
                self.step_selection(1, cx);
            } else if key == "k" || key == "up" {
                self.step_selection(-1, cx);
            } else if key == "enter" {
                if let Some(index) = self.model.selected {
                    self.select_message(index, cx);
                }
            } else if key == "v" {
                self.toggle_view(cx);
            }
            return;
        };

        match key {
            "backspace" => {
                self.field_mut(field).pop();
            }
            "enter" => {
                if matches!(field, Field::DraftBody) {
                    self.field_mut(field).push('\n');
                } else {
                    self.commit_field(field, window, cx);
                }
            }
            "tab" => {
                self.next_field(field);
            }
            "space" => {
                self.field_mut(field).push(' ');
            }
            _ => {
                if let Some(text) = keystroke.key_char.as_deref() {
                    if !text.is_empty() {
                        self.field_mut(field).push_str(text);
                    }
                }
            }
        }

        if field == Field::Search {
            self.model.selected = Some(0);
            if self.search_used && self.model.search.trim().is_empty() && !self.model.demo {
                self.model.reload();
                self.search_used = false;
            }
        }
        cx.notify();
    }

    fn field_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Search => &mut self.model.search,
            Field::Email => &mut self.model.sign_in.email,
            Field::Password => &mut self.model.sign_in.password,
            Field::DraftTo => &mut self.model.draft_to,
            Field::DraftSubject => &mut self.model.draft_subject,
            Field::DraftBody => &mut self.model.draft_body,
            Field::EventTitle => &mut self.model.event_draft.title,
            Field::EventDate => &mut self.model.event_draft.day,
            Field::EventStart => &mut self.model.event_draft.start,
            Field::EventEnd => &mut self.model.event_draft.end,
        }
    }

    fn next_field(&mut self, field: Field) {
        self.model.focused = match field {
            Field::Email => Some(Field::Password),
            Field::DraftTo => Some(Field::DraftSubject),
            Field::DraftSubject => Some(Field::DraftBody),
            Field::Password => Some(Field::Email),
            Field::EventTitle => Some(Field::EventDate),
            Field::EventDate => Some(Field::EventStart),
            Field::EventStart => Some(Field::EventEnd),
            Field::EventEnd => Some(Field::EventTitle),
            Field::Search | Field::DraftBody => None,
        };
    }

    fn commit_field(&mut self, field: Field, _window: &mut Window, cx: &mut Context<Self>) {
        match field {
            Field::Email => {
                self.model.focused = Some(Field::Password);
                cx.notify();
            }
            Field::Password => {
                self.start_sign_in(cx);
            }
            Field::DraftBody => {
                self.send_draft(cx);
            }
            Field::Search => {
                self.model.focused = None;
                self.search_now(cx);
            }
            field @ (Field::EventTitle | Field::EventDate | Field::EventStart | Field::EventEnd) => {
                self.next_field(field);
                cx.notify();
            }
            _ => {}
        }
    }

    fn focus_field(&mut self, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        self.model.focused = Some(field);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn step_selection(&mut self, delta: i32, cx: &mut Context<Self>) {
        if self.model.messages.is_empty() {
            return;
        }
        let len = self.model.messages.len();
        let current = self.model.selected.unwrap_or(0);
        let next = (current as i32 + delta).clamp(0, len as i32 - 1) as usize;
        self.select_message(next, cx);
    }

    /// Open a message: update selection, mark it read and fetch a missing body.
    fn select_message(&mut self, index: usize, cx: &mut Context<Self>) {
        self.model.select_message(index);
        self.model.mark_seen(index);
        cx.notify();

        if self.model.demo {
            return;
        }
        let Some(message) = self.model.messages.get(index).cloned() else {
            return;
        };
        if message.body.is_some() || message.row.body_text.is_some() {
            return;
        }
        let id = message.row.id;
        if !self.fetching.insert(id) {
            return;
        }

        let config = self.model.config.clone();
        let Some(account) = self.model.account().cloned() else {
            self.fetching.remove(&id);
            return;
        };
        let folder = message.row.folder.clone();
        let uid = message.row.uid;

        let task = cx.background_executor().spawn(async move {
            let db = crate::db::Db::open_default()?;
            crate::mail::sync::fetch_body(&config, &db, &account, &folder, uid)
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.fetching.remove(&id);
                match result {
                    Ok((text, html)) => {
                        if let Some(position) = this
                            .model
                            .messages
                            .iter()
                            .position(|message| message.row.id == id)
                        {
                            this.model.set_body(position, text, html);
                        }
                        cx.notify();
                    }
                    Err(error) => this.toast(format!("Could not load message: {error:#}"), cx),
                }
            })
            .ok();
        })
        .detach();
    }

    /// Run a cache-wide full-text search (the inline filter stays instant).
    fn search_now(&mut self, cx: &mut Context<Self>) {
        let query = self.model.search.trim().to_string();
        if self.model.demo || query.is_empty() {
            self.search_used = false;
            if !self.model.demo {
                self.model.reload();
            }
            cx.notify();
            return;
        }

        self.search_used = true;
        let task = cx
            .background_executor()
            .spawn(async move { crate::db::Db::open_default()?.search(&query, 200) });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(rows) => this.model.set_search_results(rows),
                    Err(error) => this.toast(format!("Search failed: {error:#}"), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.model.show_settings = true;
        self.model.show_account_menu = false;
        cx.notify();
    }

    fn toggle_account_menu(&mut self, cx: &mut Context<Self>) {
        self.model.show_account_menu = !self.model.show_account_menu;
        cx.notify();
    }

    fn switch_account(&mut self, id: String, cx: &mut Context<Self>) {
        self.model.set_active_account(id);
        self.model.show_account_menu = false;
        cx.notify();
    }

    fn open_account_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_tab = 1;
        self.open_settings(cx);
    }

    fn add_account(&mut self, cx: &mut Context<Self>) {
        self.model.show_settings = false;
        self.model.show_account_menu = false;
        self.model.show_account_manager = true;
        self.model.sign_in.error = None;
        self.model.sign_in.busy = false;
        cx.notify();
    }

    fn close_account_manager(&mut self, cx: &mut Context<Self>) {
        self.model.show_account_manager = false;
        self.model.focused = None;
        cx.notify();
    }

    fn choose_provider(&mut self, provider: crate::config::Provider, cx: &mut Context<Self>) {
        self.model.sign_in.provider = Some(provider);
        self.model.sign_in.error = None;
        self.model.sign_in.busy = false;
        self.model.sign_in.email.clear();
        self.model.sign_in.password.clear();
        self.model.focused = if provider == crate::config::Provider::Google {
            None
        } else {
            Some(Field::Email)
        };
        cx.notify();
    }

    fn toggle_view(&mut self, cx: &mut Context<Self>) {
        self.model.view = match self.model.view {
            MailView::Mail => MailView::Calendar,
            MailView::Calendar => MailView::Mail,
        };
        cx.notify();
    }

    fn set_mode(&mut self, mode: crate::theme::ThemeMode, cx: &mut Context<Self>) {
        self.model.set_mode(mode);
        self.model.save();
        sync_component_theme(&self.model, cx);
        cx.notify();
    }

    fn set_accent(&mut self, accent: Accent, cx: &mut Context<Self>) {
        self.model.set_accent(accent);
        self.model.save();
        sync_component_theme(&self.model, cx);
        cx.notify();
    }

    fn use_saved_account(&mut self, cx: &mut Context<Self>) {
        self.model.load_saved_credentials();
        if self.model.sign_in.password.is_empty() {
            self.toast("No saved credentials found", cx);
            return;
        }
        self.start_sign_in(cx);
    }

    fn cancel_sign_in(&mut self, cx: &mut Context<Self>) {
        if let Some(flag) = self.sign_in_cancel.as_ref() {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.sign_in_cancel = None;
        self.model.sign_in.busy = false;
        self.model.sign_in.error = None;
        cx.notify();
    }

    fn start_sign_in(&mut self, cx: &mut Context<Self>) {
        let provider = self.model.sign_in.provider;
        let Some(provider) = provider else { return };
        if self.model.sign_in.busy {
            return;
        }

        self.model.sign_in.busy = true;
        self.model.sign_in.error = None;
        let cancel = Arc::new(AtomicBool::new(false));
        self.sign_in_cancel = Some(cancel.clone());
        cx.notify();

        let mut config = self.model.config.clone();
        let email = self.model.sign_in.email.trim().to_string();
        let password = self.model.sign_in.password.clone();
        let client_id = crate::config::GoogleConfig::embedded_client_id()
            .unwrap_or_default()
            .to_string();
        let client_secret = crate::config::GoogleConfig::embedded_client_secret()
            .unwrap_or_default()
            .to_string();

        // Open the Google consent page immediately, on the UI thread, before
        // any keyring or disk work can delay it.
        let prepared_flow = if provider == crate::config::Provider::Google
            && !client_id.is_empty()
            && !client_secret.is_empty()
        {
            match crate::mail::oauth::begin_google_auth(&client_id) {
                Ok(flow) => Some(flow),
                Err(error) => {
                    self.model.sign_in.busy = false;
                    self.sign_in_cancel = None;
                    self.model.sign_in.error = Some(format!("{error:#}"));
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };

        let task = cx.background_executor().spawn(async move {
            let result = match provider {
                crate::config::Provider::Google => {
                    if let Some(flow) = prepared_flow {
                        crate::mail::auth::connect_google_with_flow(
                            &mut config,
                            flow,
                            &client_id,
                            &client_secret,
                            None,
                            Some(cancel.clone()),
                        )
                        .map(|account| (config.clone(), account))
                    } else {
                        Err(anyhow::anyhow!(
                            "This build has no Google client configured. Rebuild with build-google.sh."
                        ))
                    }
                }
                crate::config::Provider::Apple => {
                    crate::mail::auth::connect_apple_with_password(&mut config, &email, &password)
                        .map(|account| (config.clone(), account))
                }
                crate::config::Provider::Proton => {
                    crate::mail::auth::connect_proton_with_password(&mut config, &email, &password)
                        .map(|account| (config.clone(), account))
                }
            };
            result
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.model.sign_in.busy = false;
                this.sign_in_cancel = None;
                match result {
                    Ok((config, account)) => {
                        this.model.config = config;
                        this.model.sign_in.password.clear();
                        this.model.sign_in.error = None;
                        this.model.setup_step = SetupStep::Ready;
                        this.model.screen = Screen::Mail;
                        this.model.show_account_manager = false;
                        this.model.show_settings = false;
                        this.toast(format!("Connected {}", account.email), cx);
                        this.model.reload();
                    }
                    Err(error) => {
                        this.model.sign_in.error = Some(format!("{error:#}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn finish_setup(&mut self, cx: &mut Context<Self>) {
        self.model.config.onboarding.complete = true;
        self.model.save();
        self.model.screen = Screen::Mail;
        cx.notify();
    }

    fn replay_setup(&mut self, cx: &mut Context<Self>) {
        self.model.show_settings = false;
        self.model.show_event_sheet = false;
        self.model.focused = None;
        self.model.setup_step = crate::ui::model::SetupStep::Appearance;
        self.model.screen = Screen::Setup;
        cx.notify();
    }

    fn disconnect_account(&mut self, account_id: String, cx: &mut Context<Self>) {
        let mut config = self.model.config.clone();
        let task = cx.background_executor().spawn(async move {
            crate::mail::auth::disconnect(&mut config, &account_id).map(|_| config)
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(config) => {
                        this.model.config = config;
                        this.model.show_settings = false;
                        this.toast("Account disconnected", cx);
                        if this.model.config.accounts.is_empty() {
                            this.model.load_demo();
                        } else {
                            this.model.reload();
                        }
                    }
                    Err(error) => this.toast(format!("{error:#}"), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn sync_now(&mut self, cx: &mut Context<Self>) {
        if self.model.syncing || self.model.config.accounts.is_empty() {
            return;
        }
        self.model.syncing = true;
        self.model.sync_note = Some("Syncing…".into());
        self.toast("Syncing mail…", cx);
        cx.notify();

        let config = self.model.config.clone();
        let accounts = config.accounts.clone();
        let task = cx.background_executor().spawn(async move {
            let mut total = 0;
            let mut failures = 0;
            let db = crate::db::Db::open_default();
            for account in &accounts {
                let result = match &db {
                    Ok(db) => crate::mail::sync::sync_account(&config, db, account, 50),
                    Err(error) => Err(anyhow::anyhow!(error.to_string())),
                };
                match result {
                    Ok(report) => total += report.fetched,
                    Err(_) => failures += 1,
                }
            }
            (total, failures)
        });

        cx.spawn(async move |this, cx| {
            let (total, failures) = task.await;
            this.update(cx, |this, cx| {
                this.model.syncing = false;
                this.model.reload();
                let note = match failures {
                    0 => format!("Synced · {total} new message{}", if total == 1 { "" } else { "s" }),
                    _ => format!("Synced · {total} new · {failures} account(s) failed"),
                };
                this.model.sync_note = Some(note.clone());
                this.toast(note, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn send_draft(&mut self, cx: &mut Context<Self>) {
        if self.model.draft_to.trim().is_empty() {
            self.toast("Add a recipient first", cx);
            return;
        }

        let Some(account) = self.model.account().cloned() else {
            self.toast("Connect an account to send mail", cx);
            return;
        };

        let config = self.model.config.clone();
        let draft = crate::mail::Draft {
            from: crate::mail::MailAddress {
                name: account.display_name.clone(),
                email: account.email.clone(),
            },
            to: parse_addresses(&self.model.draft_to),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: self.model.draft_subject.clone(),
            body_text: self.model.draft_body.clone(),
            body_html: None,
            in_reply_to: None,
            references: Vec::new(),
        };
        let cc: Vec<crate::mail::MailAddress> = Vec::new();
        let _ = cc;

        let task = cx.background_executor().spawn(async move {
            let credential = crate::mail::auth::credential(&account, &config)?;
            let secret = match credential {
                crate::mail::imap::Credential::Password(value) => value,
                crate::mail::imap::Credential::OAuth(value) => value,
            };
            crate::mail::smtp::send(&account, &secret, &draft)
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.toast("Sent", cx);
                        this.model.reader = ReaderMode::Read;
                        this.model.draft_body.clear();
                    }
                    Err(error) => this.toast(format!("{error:#}"), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

pub fn parse_addresses(value: &str) -> Vec<crate::mail::MailAddress> {
    value
        .split([',', ';'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            if let (Some(start), Some(end)) = (part.find('<'), part.find('>')) {
                let name = part[..start].trim().trim_matches('"').to_string();
                let email = part[start + 1..end].trim().to_string();
                crate::mail::MailAddress { name, email }
            } else {
                crate::mail::MailAddress {
                    name: String::new(),
                    email: part.to_string(),
                }
            }
        })
        .collect()
}

impl Render for CleanApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.model.palette.clone();

        div()
            .id("clean-root")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(palette.canvas.gpui())
            .text_color(palette.text.gpui())
            .font_family("Inter")
            .text_size(px(13.0))
            .on_key_down(cx.listener(Self::on_key))
            .on_action(cx.listener(|this, _: &Compose, _, cx| {
                this.model.reader = ReaderMode::Compose;
                this.model.focused = Some(Field::DraftTo);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, _, cx| this.open_settings(cx)))
            .on_action(cx.listener(|this, _: &SyncNow, _, cx| this.sync_now(cx)))
            .on_action(cx.listener(|this, _: &ToggleView, _, cx| this.toggle_view(cx)))
            .on_action(cx.listener(|this, _: &CloseOverlay, _, cx| {
                this.model.show_settings = false;
                this.model.reader = ReaderMode::Read;
                cx.notify();
            }))
            .child(match self.model.screen {
                Screen::Setup => setup::setup(self, cx).into_any_element(),
                Screen::Mail => shell::shell(self, cx).into_any_element(),
            })
    }
}

fn register_fonts(cx: &mut App) {
    let fonts = vec![
        Cow::Borrowed(include_bytes!("../../assets/fonts/Inter-Regular.ttf").as_slice()),
        Cow::Borrowed(include_bytes!("../../assets/fonts/Inter-Medium.ttf").as_slice()),
        Cow::Borrowed(include_bytes!("../../assets/fonts/Inter-SemiBold.ttf").as_slice()),
        Cow::Borrowed(include_bytes!("../../assets/fonts/Inter-Bold.ttf").as_slice()),
        Cow::Borrowed(include_bytes!("../../assets/fonts/InstrumentSerif-Regular.ttf").as_slice()),
    ];
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        tracing::warn!("could not register bundled fonts: {error}");
    }
}

fn sync_component_theme(model: &Model, cx: &mut App) {
    sync_component_theme_from_palette(&model.palette, cx);
}

fn sync_component_theme_from_palette(palette: &Palette, cx: &mut App) {
    use gpui_kit::component::{Theme, ThemeMode as KitThemeMode};

    let mode = if palette.is_dark {
        KitThemeMode::Dark
    } else {
        KitThemeMode::Light
    };
    Theme::change(mode, None, cx);
    Theme::update(cx, |theme| {
        theme.font_family = "Inter".into();
        theme.radius = px(8.0);
        theme.radius_lg = px(12.0);
        theme.window_radius = px(12.0);
        theme.colors.background = palette.canvas.gpui().into();
        theme.colors.foreground = palette.text.gpui().into();
        theme.colors.border = palette.border.gpui().into();
        theme.colors.accent = palette.accent.gpui().into();
    });
}

pub fn run() -> anyhow::Result<()> {
    run_inner(false)
}

/// Launch straight into the first-run setup screen (for testing).
pub fn run_setup() -> anyhow::Result<()> {
    run_inner(true)
}

fn run_inner(force_setup: bool) -> anyhow::Result<()> {
    application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            init(cx);
            register_fonts(cx);

            let display = cx
                .displays()
                .first()
                .map(|display| display.bounds())
                .unwrap_or_else(|| Bounds::new(point(px(0.0), px(0.0)), size(px(1536.0), px(864.0))));
            let margin = px(32.0);
            let available = size(
                display.size.width - margin * 2.0,
                display.size.height - margin * 2.0,
            );
            let window_size = size(px(1440.0), px(900.0)).min(&available);
            let origin = point(
                display.origin.x + (display.size.width - window_size.width) / 2.0,
                display.origin.y + (display.size.height - window_size.height) / 2.0,
            );
            let bounds = Bounds::new(origin, window_size);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("clean".into()),
                    appears_transparent: true,
                    traffic_light_position: None,
                }),
                window_min_size: Some(size(px(900.0), px(600.0))),
                window_decorations: Some(WindowDecorations::Client),
                app_id: Some("dev.alvyn.clean".into()),
                is_resizable: true,
                ..Default::default()
            };

            open_window(options, cx, |window, cx| {
                let app = if force_setup {
                    cx.new(CleanApp::new_with_setup)
                } else {
                    cx.new(CleanApp::new)
                };
                let palette = app.read(cx).model.palette.clone();
                sync_component_theme_from_palette(&palette, cx);
                let handle = app.read(cx).focus.clone();
                window.focus(&handle, cx);
                app
            })
            .expect("failed to open the clean window");
        });
    Ok(())
}
