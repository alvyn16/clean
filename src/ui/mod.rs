mod calendar_rail;
mod recipients;
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

/// An action applied to the currently selected message, mirrored to IMAP,
/// the local cache and the in-memory model.
#[derive(Clone, Debug)]
pub enum MessageAction {
    Archive,
    Trash,
    MoveTo(String),
    MarkUnread(bool),
    Star(bool),
}

pub struct CleanApp {
    pub model: Model,
    pub focus: FocusHandle,
    pub hovered: Option<usize>,
    pub settings_tab: usize,
    pub notifications: [bool; 3],
    pub fetching: HashSet<i64>,
    pub search_used: bool,
    pub avatars: std::collections::HashMap<String, std::path::PathBuf>,
    pub sender_avatars: std::collections::HashMap<String, std::path::PathBuf>,
    pub email_images: std::collections::HashMap<String, std::path::PathBuf>,
    pub rendered_emails: std::collections::HashMap<String, std::path::PathBuf>,
    pub message_render_paths: std::collections::HashMap<i64, std::path::PathBuf>,
    /// Live CDP-backed page for the currently open email. Renders identically
    /// to the cached screenshot but supports real mouse text selection.
    pub live_view: Option<std::sync::Arc<std::sync::Mutex<crate::mail::live_view::LiveEmailView>>>,
    pub live_view_message: Option<i64>,
    pub live_capture_path: Option<std::path::PathBuf>,
    pub live_dragging: bool,
    pub live_load_gen: u64,
    pub sign_in_cancel: Option<Arc<AtomicBool>>,
    /// Autosave generation for the composer; a stale timer is ignored.
    pub draft_save_gen: u64,
    /// Signature of the last content uploaded to the Drafts folder.
    pub draft_last_saved_sig: Option<String>,
    /// Prevent overlapping draft uploads to the IMAP server.
    pub draft_uploading: bool,
    pub draft_upload_pending: bool,
    /// Bumped when a draft is sent or discarded so late uploads can clean up.
    pub draft_upload_epoch: u64,
    /// Search generation so a late server search can't override a newer query.
    pub search_gen: u64,
    /// Attachments decoded from the currently open message.
    pub attachments: Vec<crate::db::AttachmentRow>,
    /// Files queued in the composer.
    pub compose_attachments: Vec<crate::mail::OutgoingAttachment>,
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
            sender_avatars: Self::load_cached_sender_avatars(),
            email_images: Self::load_cached_email_images(),
            rendered_emails: std::collections::HashMap::new(),
            message_render_paths: std::collections::HashMap::new(),
            live_view: None,
            live_view_message: None,
            live_capture_path: None,
            live_dragging: false,
            live_load_gen: 0,
            sign_in_cancel: None,
            draft_save_gen: 0,
            draft_last_saved_sig: None,
            draft_uploading: false,
            draft_upload_pending: false,
            draft_upload_epoch: 0,
            search_gen: 0,
            attachments: Vec::new(),
            compose_attachments: Vec::new(),
        };
        app.watch_omarchy_theme(cx);
        app.load_avatars(cx);
        app.load_sender_avatars(cx);
        app.start_live_sync(cx);
        app.start_idle_listeners(cx);
        let initial_id = app.model.messages.get(app.model.selected.unwrap_or(0)).map(|m| m.row.id);
        let initial_html = app
            .model
            .messages
            .get(app.model.selected.unwrap_or(0))
            .and_then(|m| m.row.body_html.clone());
        if let Some(id) = initial_id {
            app.refresh_attachments(id, cx);
            app.load_email_images(id, initial_html.as_deref(), cx);
            app.load_rendered_email(id, initial_html.as_deref(), cx);
            if let Some(html) = initial_html.clone() {
                app.load_live_view(id, html, cx);
            }
        }
        app
    }

    /// Cached avatar path for a given account, if available.
    pub fn avatar_for(&self, account_id: &str) -> Option<std::path::PathBuf> {
        self.avatars.get(account_id).cloned()
    }

    /// Cached email image paths keyed by URL.
    fn load_cached_email_images() -> std::collections::HashMap<String, std::path::PathBuf> {
        let mut map = std::collections::HashMap::new();
        let dir = crate::config::data_dir().join("email-images");
        let Ok(entries) = std::fs::read_dir(dir) else {
            return map;
        };
        // We can't reverse the hash, so store by filename stem for later matching.
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(hash) = name.strip_suffix(".img") {
                map.insert(hash.to_string(), entry.path());
            }
        }
        map
    }

    pub fn email_image_for(&self, url: &str) -> Option<std::path::PathBuf> {
        use md5::Digest;
        let hash: String = md5::Md5::digest(url.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        self.email_images.get(&hash).cloned()
    }

    /// Load an HTML email into the live CDP page so its text is selectable,
    /// then capture the first screenshot for display. The same Chromium page
    /// is reused across emails — starting a new browser for every message
    /// leaks renderer processes and hundreds of MB of profile data.
    pub fn load_live_view(&mut self, message_id: i64, html: String, cx: &mut Context<Self>) {
        if self.live_view_message == Some(message_id)
            && self.live_view.is_some()
            && self.live_capture_path.is_some()
        {
            return;
        }
        self.live_load_gen = self.live_load_gen.wrapping_add(1);
        let load_gen = self.live_load_gen;
        self.live_view_message = Some(message_id);
        if let Some(old) = &self.live_capture_path {
            let _ = std::fs::remove_file(old);
        }
        self.live_capture_path = None;
        let shared = self.live_view.clone();
        let task = cx.background_executor().spawn(async move {
            let view = match shared {
                Some(view) => view,
                None => match crate::mail::live_view::LiveEmailView::start() {
                    Ok(view) => Arc::new(std::sync::Mutex::new(view)),
                    Err(error) => {
                        tracing::warn!("live view start failed: {error:#}");
                        return None;
                    }
                },
            };
            // Hold the lock across load + capture so a rapid second navigation
            // can't make this capture show the wrong email.
            let result: Result<std::path::PathBuf, String> = (|| {
                let mut guard = view.lock().map_err(|_| "live view lock poisoned".to_string())?;
                guard.load(&html, None).map_err(|e| e.to_string())?;
                guard.capture().map_err(|e| e.to_string())
            })();
            match result {
                Ok(path) => Some((view, path)),
                Err(error) => {
                    tracing::warn!("live view load failed: {error}");
                    drop(view);
                    None
                }
            }
        });
        cx.spawn(async move |this, cx| {
            match task.await {
                Some((view, path)) => {
                    this.update(cx, |this, cx| {
                        if this.live_load_gen == load_gen {
                            this.live_view = Some(view);
                            this.live_capture_path = Some(path);
                        } else {
                            let _ = std::fs::remove_file(&path);
                        }
                        cx.notify();
                    })
                    .ok();
                }
                None => {
                    this.update(cx, |this, cx| {
                        if this.live_load_gen == load_gen {
                            this.live_view = None;
                            this.live_capture_path = None;
                            cx.notify();
                        }
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Currently selected text in the live page (for Ctrl+C).
    fn live_selected_text(&self) -> Option<String> {
        let view = self.live_view.as_ref()?;
        let v = view.lock().ok()?;
        v.selected_text().ok()
    }

    /// Re-capture the live page after a selection interaction.
    pub fn recapture_live(&mut self, cx: &mut Context<Self>) {
        let view = self.live_view.clone();
        if view.is_none() {
            return;
        }
        let task = cx.background_executor().spawn(async move {
            let view = view?;
            let mut v = view.lock().ok()?;
            v.capture().ok()
        });
        cx.spawn(async move |this, cx| {
            if let Some(path) = task.await {
                this.update(cx, |this, cx| {
                    if let Some(old) = &this.live_capture_path {
                        if old != &path {
                            let _ = std::fs::remove_file(old);
                        }
                    }
                    this.live_capture_path = Some(path);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Refresh the cached attachment list for the open message.
    pub fn refresh_attachments(&mut self, message_id: i64, cx: &mut Context<Self>) {
        self.attachments = self
            .model
            .db
            .as_ref()
            .and_then(|db| db.attachments_for(message_id).ok())
            .unwrap_or_default();
        cx.notify();
    }

    /// Open a cached attachment with the desktop's default application.
    pub fn open_attachment(&self, index: usize) {
        if let Some(item) = self.attachments.get(index) {
            crate::mail::render::open_in_browser(&item.path);
        }
    }

    /// Copy a cached attachment into Downloads and report where it landed.
    pub fn save_attachment(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(item) = self.attachments.get(index) else {
            return;
        };
        match crate::mail::attachments::download(item) {
            Ok(path) => self.toast(format!("Saved to {}", path.display()), cx),
            Err(error) => self.toast(format!("Could not save attachment: {error:#}"), cx),
        }
    }

    /// Queue local files for the composer and persist them with the draft.
    pub fn attach_files(&mut self, paths: Vec<std::path::PathBuf>, cx: &mut Context<Self>) {
        for path in paths {
            if !path.is_file() {
                continue;
            }
            let filename = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "attachment".into());
            if self
                .compose_attachments
                .iter()
                .any(|item| item.path == path)
            {
                continue;
            }
            let mime = crate::mail::attachments::mime_for_path(&path);
            let size = std::fs::metadata(&path).map(|m| m.len() as usize).unwrap_or(0);
            self.compose_attachments.push(crate::mail::OutgoingAttachment {
                filename,
                mime,
                size,
                path,
            });
        }
        if let Some(id) = self.model.draft_id {
            self.persist_draft_attachments(id);
        }
        self.toast(
            format!("{} attachment(s) added", self.compose_attachments.len()),
            cx,
        );
    }

    fn persist_draft_attachments(&self, draft_id: i64) {
        let Some(db) = self.model.db.as_ref() else {
            return;
        };
        let rows: Vec<crate::db::DraftAttachmentRow> = self
            .compose_attachments
            .iter()
            .map(|item| crate::db::DraftAttachmentRow {
                filename: item.filename.clone(),
                mime: item.mime.clone(),
                size: item.size,
                path: item.path.to_string_lossy().into_owned(),
            })
            .collect();
        if let Err(error) = db.set_draft_attachments(draft_id, &rows) {
            tracing::warn!("could not save draft attachments: {error:#}");
        }
    }

    pub fn remove_compose_attachment(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.compose_attachments.len() {
            self.compose_attachments.remove(index);
            if let Some(id) = self.model.draft_id {
                self.persist_draft_attachments(id);
            }
            self.toast("Attachment removed", cx);
        }
    }

    /// Download images found in the current email so the reader can show them.
    pub fn load_email_images(&mut self, message_id: i64, html: Option<&str>, cx: &mut Context<Self>) {
        let Some(html) = html else { return };
        if !crate::config::Config::load()
            .map(|c| c.appearance.load_remote_images)
            .unwrap_or(true)
        {
            return;
        }
        // Scan raw HTML for image URLs (the block parser only sees some).
        let urls = crate::mail::images::extract_image_urls(html);
        let pending: Vec<String> = urls
            .into_iter()
            .filter(|u| !crate::mail::images::cache_path(u).exists())
            .collect();
        if pending.is_empty() {
            // Images are all cached — render only if we don't already have a
            // render built from this exact localized HTML.
            let expected = crate::mail::render::cache_path(html);
            if self.current_render_path(message_id).as_deref() != Some(expected.as_path()) {
                let owned = html.to_string();
                self.rerender_email(message_id, Some(&owned), cx);
            }
            return;
        }
        let owned_html = html.to_string();
        let msg_id = message_id;
        let total = pending.len();
        let completed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for url in pending {
            let done = completed.clone();
            let h = owned_html.clone();
            let task = cx.background_executor()
                .spawn(async move { crate::mail::images::cache_image(&url).is_ok() });
            cx.spawn(async move |this, cx| {
                let _ = task.await;
                let count = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                this.update(cx, |this, cx| {
                    cx.notify();
                    // The render cache key already includes image availability,
                    // so the next lookup naturally produces a fresh render. Do
                    // NOT clear all renders — that makes every email flash.
                    if count == total {
                        this.rerender_email(msg_id, Some(&h), cx);
                    }
                })
                .ok();
            })
            .detach();
        }
    }

    fn current_render_path(&self, id: i64) -> Option<std::path::PathBuf> {
        self.message_render_paths.get(&id).cloned()
    }

    fn db_row(&self, id: i64) -> Option<crate::db::MessageRow> {
        self.model
            .db
            .as_ref()
            .and_then(|db| db.message(id).ok().flatten())
    }

    /// Get the best available render for a message (stable across re-renders).
    pub fn render_for_message(&self, message_id: i64, html: &str) -> Option<std::path::PathBuf> {
        if let Some(path) = self.message_render_paths.get(&message_id) {
            return Some(path.clone());
        }
        let cached = crate::mail::render::cache_path(html);
        if cached.exists() {
            return Some(cached);
        }
        None
    }

    /// Render an HTML email to a PNG via headless Chromium (full CSS fidelity).
    pub fn load_rendered_email(&mut self, message_id: i64, html: Option<&str>, cx: &mut Context<Self>) {
        self.load_rendered_email_inner(message_id, html, false, cx);
    }

    /// Force a fresh render (used after new images are downloaded).
    fn rerender_email(&mut self, message_id: i64, html: Option<&str>, cx: &mut Context<Self>) {
        self.load_rendered_email_inner(message_id, html, true, cx);
    }

    fn load_rendered_email_inner(&mut self, message_id: i64, html: Option<&str>, force: bool, cx: &mut Context<Self>) {
        let Some(html) = html else { return };
        if html.len() < 64 {
            return;
        }
        let path = crate::mail::render::cache_path(html);
        if path.exists() && !force {
            self.message_render_paths.insert(message_id, path);
            return;
        }
        let owned = html.to_string();
        let task = cx.background_executor().spawn(async move {
            crate::mail::render::render_email(&owned, None).ok()
        });
        cx.spawn(async move |this, cx| {
            if let Some(path) = task.await {
                this.update(cx, |this, cx| {
                    // Remove the previous render so render_for_message can't
                    // keep returning the stale one (e.g. gray boxes before images).
                    if let Some(old) = this.message_render_paths.insert(message_id, path.clone()) {
                        if old != path {
                            let _ = std::fs::remove_file(&old);
                        }
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Cached avatar path for a message sender, if available.
    pub fn sender_avatar_for(&self, email: &str) -> Option<std::path::PathBuf> {
        self.sender_avatars.get(&email.to_ascii_lowercase()).cloned()
    }

    fn load_cached_sender_avatars() -> std::collections::HashMap<String, std::path::PathBuf> {
        let mut map = std::collections::HashMap::new();
        let Ok(config) = crate::config::Config::load() else {
            return map;
        };
        let Ok(db) = crate::db::Db::open_default() else {
            return map;
        };
        let mut seen = std::collections::HashSet::new();
        for account in &config.accounts {
            for folder in db.all_folders().ok().unwrap_or_default() {
                let Ok(rows) = db.list_messages(&account.id, &folder.name, 1000, 0) else {
                    continue;
                };
                for row in rows {
                    let email = row.from_addr.trim().to_ascii_lowercase();
                    if email.is_empty() || !seen.insert(email.clone()) {
                        continue;
                    }
                    let path = crate::mail::avatars::cache_path(&email);
                    if path.exists() {
                        map.insert(email, path);
                    }
                }
            }
        }
        map
    }

    /// Fetch real avatars for every sender currently in the message list.
    pub fn load_sender_avatars(&mut self, cx: &mut Context<Self>) {
        let mut emails: Vec<String> = self
            .model
            .messages
            .iter()
            .map(|m| m.row.from_addr.trim().to_ascii_lowercase())
            .filter(|e| !e.is_empty() && !self.sender_avatars.contains_key(e))
            .collect();
        emails.sort();
        emails.dedup();

        for email in emails {
            let task = cx.background_executor().spawn(async move {
                crate::mail::avatars::fetch_sender_avatar(&email).ok().map(|path| (email, path))
            });
            cx.spawn(async move |this, cx| {
                if let Some((email, path)) = task.await {
                    this.update(cx, |this, cx| {
                        this.sender_avatars.insert(email, path);
                        cx.notify();
                    })
                    .ok();
                }
            })
            .detach();
        }
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
        let command = keystroke.modifiers.control;

        if key == "escape" {
            if self.model.show_narrow_sidebar {
                self.model.show_narrow_sidebar = false;
                cx.notify();
                return;
            }
            if self.model.show_message_menu {
                self.model.show_message_menu = false;
                cx.notify();
                return;
            }
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
                self.reload_mail(cx);
                self.search_used = false;
            }
            if self.model.show_settings {
                self.model.show_settings = false;
            } else if matches!(self.model.reader, ReaderMode::Reply | ReaderMode::Compose) {
                self.close_composer(cx);
                return;
            } else if self.model.focused.is_some() {
                self.model.focused = None;
            }
            cx.notify();
            return;
        }

        if command {
            match key {
                "k" => {
                    self.model.focused = Some(Field::Search);
                    window.focus(&self.focus, cx);
                }
                "n" => {
                    self.begin_compose(cx);
                    window.focus(&self.focus, cx);
                }
                "r" => self.sync_now(cx),
                "c" => {
                    // Ctrl+C copies live email selection; otherwise leave the
                    // keystroke to a focused text field.
                    if let Some(text) = self.live_selected_text() {
                        if !text.is_empty() {
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                            self.toast("Copied selected text", cx);
                            return;
                        }
                    }
                }
                "a" => {
                    // Ctrl+A selects all text in the live email.
                    if self.live_view.is_some() {
                        if let Some(view) = self.live_view.clone() {
                            if let Ok(v) = view.lock() {
                                let _ = v.select_all();
                            }
                        }
                        self.recapture_live(cx);
                        return;
                    }
                }
                "enter" if self.model.focused == Some(Field::DraftBody) => {
                    self.send_draft(cx);
                    return;
                }
                "," => self.open_settings(cx),
                "m" => self.toggle_view(cx),
                _ => return,
            }
            cx.notify();
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
            } else if key == "e" {
                self.message_action(MessageAction::Archive, cx);
            } else if key == "#" || key == "delete" {
                self.message_action(MessageAction::Trash, cx);
            } else if key == "u" {
                let unread = self
                    .model
                    .current()
                    .map(|m| m.unread)
                    .unwrap_or(false);
                self.message_action(MessageAction::MarkUnread(!unread), cx);
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
                self.reload_mail(cx);
                self.search_used = false;
            }
        }

        if matches!(
            field,
            Field::DraftTo | Field::DraftSubject | Field::DraftBody | Field::AttachmentPath
        ) {
            self.schedule_draft_save(cx);
        }
        cx.notify();
    }

    fn field_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Search => &mut self.model.search,
            Field::Email => &mut self.model.sign_in.email,
            Field::Password => &mut self.model.sign_in.password,
            Field::DraftTo => &mut self.model.draft_to,
            Field::DraftCc => &mut self.model.draft_cc,
            Field::DraftBcc => &mut self.model.draft_bcc,
            Field::DraftSubject => &mut self.model.draft_subject,
            Field::DraftBody => &mut self.model.draft_body,
            Field::AttachmentPath => &mut self.model.attachment_path,
            Field::EventTitle => &mut self.model.event_draft.title,
            Field::EventDate => &mut self.model.event_draft.day,
            Field::EventStart => &mut self.model.event_draft.start,
            Field::EventEnd => &mut self.model.event_draft.end,
        }
    }

    fn next_field(&mut self, field: Field) {
        self.model.focused = match field {
            Field::Email => Some(Field::Password),
            Field::DraftTo => Some(Field::DraftCc),
            Field::DraftCc => Some(Field::DraftBcc),
            Field::DraftBcc => Some(Field::DraftSubject),
            Field::DraftSubject => Some(Field::DraftBody),
            Field::Password => Some(Field::Email),
            Field::EventTitle => Some(Field::EventDate),
            Field::EventDate => Some(Field::EventStart),
            Field::EventStart => Some(Field::EventEnd),
            Field::EventEnd => Some(Field::EventTitle),
            Field::Search | Field::AttachmentPath => None,
            Field::DraftBody => Some(Field::AttachmentPath),
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
            field @ (Field::DraftTo | Field::DraftCc | Field::DraftBcc | Field::DraftSubject) => {
                self.next_field(field);
                cx.notify();
            }
            Field::AttachmentPath => {
                let path = std::path::PathBuf::from(self.model.attachment_path.trim());
                self.model.attachment_path.clear();
                if path.exists() {
                    self.attach_files(vec![path], cx);
                } else {
                    self.toast("File not found", cx);
                }
                self.model.focused = Some(Field::DraftBody);
            }
            Field::Search => {
                self.model.focused = None;
                self.search_now(cx);
            }
            field @ (Field::EventTitle
            | Field::EventDate
            | Field::EventStart
            | Field::EventEnd) => {
                let _ = field;
            }
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
        if let Some(draft_id) = self.model.messages.get(index).and_then(|m| m.draft_id) {
            self.resume_draft(draft_id, cx);
            return;
        }
        self.model.select_message(index);
        self.model.mark_seen(index);
        if let Some(id) = self.model.current().map(|m| m.row.id) {
            self.refresh_attachments(id, cx);
        }
        if self.live_view_message.is_some() {
            self.live_load_gen = self.live_load_gen.wrapping_add(1);
            if let Some(old) = &self.live_capture_path {
                let _ = std::fs::remove_file(old);
            }
            // Keep the Chromium page alive; only its captured image is stale.
            self.live_view_message = None;
            self.live_capture_path = None;
            self.live_dragging = false;
        }
        // Message rows omit body columns. Load a cached body before deciding
        // to hit IMAP again, so reopening mail is instant and works offline.
        let cached = self
            .model
            .messages
            .get(index)
            .and_then(|m| self.model.db.as_ref().map(|db| (m.row.id, db)))
            .and_then(|(id, db)| db.message(id).ok().flatten())
            .filter(|row| row.body_text.is_some() || row.body_html.is_some());
        if let Some(row) = cached {
            let text = row.body_text.clone().unwrap_or_default();
            let html = row.body_html.clone();
            self.model.set_body(index, text, html);
        }
        let html = self
            .model
            .messages
            .get(index)
            .and_then(|m| m.row.body_html.clone());
        if let Some(id) = self.model.current().map(|m| m.row.id) { self.load_email_images(id, html.as_deref(), cx); }
        if let Some(id) = self.model.current().map(|m| m.row.id) {
            self.load_rendered_email(id, html.as_deref(), cx);
            if let Some(html) = html.clone() {
                self.load_live_view(id, html, cx);
            }
        }
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
                        this.refresh_attachments(id, cx);
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
        if query.is_empty() {
            self.search_gen = self.search_gen.wrapping_add(1);
            self.search_used = false;
            if !self.model.demo {
                self.reload_mail(cx);
            }
            cx.notify();
            return;
        }

        self.search_used = true;
        self.search_gen = self.search_gen.wrapping_add(1);
        let generation = self.search_gen;
        let local_query = query.clone();
        let local = cx
            .background_executor()
            .spawn(async move { crate::db::Db::open_default()?.search(&local_query, 200) });

        // Show cached hits immediately; the server search then replaces them
        // with a complete result set (including messages never downloaded).
        cx.spawn(async move |this, cx| {
            if let Ok(rows) = local.await {
                this.update(cx, |this, cx| {
                    if this.search_gen == generation {
                        this.model.set_search_results(rows);
                        if this.model.current().is_some() {
                            this.select_message(0, cx);
                        }
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();

        let account = self.model.account().cloned();
        let config = self.model.config.clone();
        let server_query = query.clone();
        let server = cx.background_executor().spawn(async move {
            let Some(account) = account else {
                return Ok(Vec::new());
            };
            let db = crate::db::Db::open_default()?;
            crate::mail::sync::search_mail(&config, &db, &account, &server_query, 200)
        });

        cx.spawn(async move |this, cx| {
            match server.await {
                Ok(rows) => {
                    this.update(cx, |this, cx| {
                        if this.search_gen == generation {
                            this.model.set_search_results(rows);
                            if this.model.current().is_some() {
                                this.select_message(0, cx);
                            }
                            cx.notify();
                        }
                    })
                    .ok();
                }
                Err(error) => {
                    tracing::warn!("server search failed: {error:#}");
                    this.update(cx, |this, cx| {
                        if this.search_gen == generation && this.model.messages.is_empty() {
                            this.toast(
                                "Server search failed — showing cached results".to_string(),
                                cx,
                            );
                        }
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Reload the message list and refresh sender avatars for the new messages.
    fn reload_mail(&mut self, cx: &mut Context<Self>) {
        self.model.reload();
        if let Some(id) = self.model.current().map(|m| m.row.id) {
            self.refresh_attachments(id, cx);
        }
        self.load_sender_avatars(cx);
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

    /// Save the active draft before leaving the composer for another view.
    fn stash_draft(&mut self, cx: &mut Context<Self>) {
        if matches!(self.model.reader, ReaderMode::Compose | ReaderMode::Reply) {
            if self.draft_has_content() {
                self.flush_draft(cx);
            } else if self.model.draft_id.is_some() {
                self.discard_draft(cx);
                return;
            }
        }
        self.model.reset_draft();
        self.compose_attachments.clear();
        self.draft_last_saved_sig = None;
    }

    fn select_folder_at(&mut self, folder: String, cx: &mut Context<Self>) {
        self.stash_draft(cx);
        self.model.show_narrow_sidebar = false;
        self.search_used = false;
        self.search_gen = self.search_gen.wrapping_add(1);
        self.model.select_folder(folder);
        self.load_sender_avatars(cx);
        let draft_selected = self
            .model
            .current()
            .map(|message| message.draft_id.is_some())
            .unwrap_or(false);
        if draft_selected {
            cx.notify();
            return;
        }
        let html = self
            .model
            .messages
            .get(self.model.selected.unwrap_or(0))
            .and_then(|m| m.row.body_html.clone());
        if let Some(id) = self.model.current().map(|m| m.row.id) { self.load_email_images(id, html.as_deref(), cx); }
        if let Some(id) = self.model.current().map(|m| m.row.id) {
            self.load_rendered_email(id, html.as_deref(), cx);
        }
        cx.notify();
    }

    fn switch_account(&mut self, id: String, cx: &mut Context<Self>) {
        self.stash_draft(cx);
        self.search_used = false;
        self.search_gen = self.search_gen.wrapping_add(1);
        self.model.set_active_account(id);
        self.model.show_account_menu = false;
        self.load_sender_avatars(cx);
        let html = self
            .model
            .messages
            .get(self.model.selected.unwrap_or(0))
            .and_then(|m| m.row.body_html.clone());
        if let Some(id) = self.model.current().map(|m| m.row.id) { self.load_email_images(id, html.as_deref(), cx); }
        if let Some(id) = self.model.current().map(|m| m.row.id) {
            self.load_rendered_email(id, html.as_deref(), cx);
        }
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
                        this.reload_mail(cx);
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
                            this.reload_mail(cx);
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

    /// IMAP IDLE listeners: one long-lived connection per account that blocks
    /// on the inbox instead of waiting for the next polling tick. Any mailbox
    /// change immediately kicks a normal sync, which imports the new message,
    /// updates counts and shows the toast. Polling remains as a fallback for
    /// other folders and for servers without IDLE support.
    fn start_idle_listeners(&mut self, cx: &mut Context<Self>) {
        for account in self.model.config.accounts.clone() {
            let config = self.model.config.clone();
            cx.spawn(async move |this, cx| {
                loop {
                    let still_configured = this.upgrade().map(|handle| {
                        handle.read_with(cx, |this: &CleanApp, _| {
                            this.model.config.accounts.iter().any(|a| a.id == account.id)
                        })
                    }).unwrap_or(false);
                    if !still_configured {
                        return;
                    }

                    let account_email = account.email.clone();
                    let account = account.clone();
                    let config = config.clone();
                    let task = cx.background_executor().spawn(async move {
                        let credential = crate::mail::auth::credential(&account, &config)?;
                        let mut session = crate::mail::imap::connect(&account, &credential)?;
                        let folder = crate::db::Db::open_default()
                            .ok()
                            .and_then(|db| db.folder_by_role(&account.id, "inbox").ok().flatten())
                            .map(|folder| folder.name)
                            .unwrap_or_else(|| "INBOX".to_string());
                        session.select(&folder)?;
                        tracing::info!("IDLE watching {} in {folder}", account.email);
                        // Reissue before the usual 30-minute server timeout.
                        let outcome = session.idle_wait(std::time::Duration::from_secs(25 * 60))?;
                        tracing::info!("IDLE {} woke: {outcome:?}", account.email);
                        session.logout();
                        Ok::<bool, anyhow::Error>(
                            outcome == crate::mail::imap::IdleOutcome::Changed,
                        )
                    });

                    match task.await {
                        Ok(true) => {
                            this.update(cx, |this, cx| {
                                if !this.model.syncing {
                                    this.sync_now(cx);
                                }
                            })
                            .ok();
                        }
                        Ok(false) => {}
                        Err(error) => {
                            tracing::warn!("IDLE for {account_email} failed: {error:#}");
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(15))
                                .await;
                        }
                    }
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(1))
                        .await;
                }
            })
            .detach();
        }
    }

    /// Background loop: check for new mail periodically so messages arrive
    /// even when IDLE is unavailable or a change happens in another folder.
    fn start_live_sync(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                let config = match this.upgrade() {
                    Some(handle) => handle.read_with(cx, |this, _| this.model.config.clone()),
                    None => return,
                };
                let accounts = config.accounts.clone();
                let task = cx.background_executor().spawn({
                    let config = config.clone();
                    async move {
                        let mut total = 0;
                        let mut removed = 0;
                        let mut updated = 0;
                        let mut failures = 0;
                        let db = crate::db::Db::open_default();
                        for account in &accounts {
                            let result = match &db {
                                Ok(db) => crate::mail::sync::sync_account(&config, db, account, 50),
                                Err(error) => Err(anyhow::anyhow!(error.to_string())),
                            };
                            match result {
                                Ok(report) => {
                                    total += report.fetched;
                                    removed += report.removed;
                                    updated += report.updated;
                                }
                                Err(_) => failures += 1,
                            }
                        }
                        (total, removed, updated, failures)
                    }
                });
                let (total, removed, updated, failures) = task.await;
                this.update(cx, |this, cx| {
                    if (total > 0 || removed > 0 || updated > 0) && !this.search_used {
                        this.reload_mail(cx);
                        // Pre-render the newest few messages so clicking is instant.
                        let top: Vec<(i64, crate::db::MessageRow)> = this
                            .model
                            .messages
                            .iter()
                            .take(3)
                            .filter_map(|m| {
                                this.db_row(m.row.id)
                                    .map(|row| (m.row.id, row))
                            })
                            .collect();
                        for (id, row) in top {
                            if let Some(html) = row.body_html.clone() {
                                // Download images first; the callback re-renders with them.
                                this.load_email_images(id, Some(&html), cx);
                            }
                        }
                        let msg = if total == 1 {
                            "1 new message".to_string()
                        } else {
                            format!("{total} new messages")
                        };
                        let msg = if removed > 0 {
                            format!("{msg} · {removed} removed")
                        } else {
                            msg
                        };
                        this.model.sync_note = Some(msg.clone());
                        this.toast(msg, cx);
                    }
                    if failures > 0 {
                        this.model.sync_note = Some(format!("Sync failed for {failures} account(s)"));
                    }
                    cx.notify();
                })
                .ok();

                let interval = this.upgrade().map(|h| {
                    h.read_with(cx, |this, _| this.model.config.appearance.sync_interval_secs.max(3))
                }).unwrap_or(10);
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(interval))
                    .await;
            }
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
            let mut removed = 0;
            let mut updated = 0;
            let mut failures = 0;
            let db = crate::db::Db::open_default();
            for account in &accounts {
                let result = match &db {
                    Ok(db) => crate::mail::sync::sync_account(&config, db, account, 50),
                    Err(error) => Err(anyhow::anyhow!(error.to_string())),
                };
                match result {
                    Ok(report) => {
                        total += report.fetched;
                        removed += report.removed;
                        updated += report.updated;
                    }
                    Err(_) => failures += 1,
                }
            }
            (total, removed, updated, failures)
        });

        cx.spawn(async move |this, cx| {
            let (total, removed, updated, failures) = task.await;
            this.update(cx, |this, cx| {
                this.model.syncing = false;
                if !this.search_used {
                    this.reload_mail(cx);
                }
                let note = match failures {
                    0 => format!(
                        "Synced · {total} new · {removed} removed · {updated} updated"
                    ),
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

    // ---- composer drafts -------------------------------------------------

    fn draft_has_content(&self) -> bool {
        !self.model.draft_is_empty() || !self.compose_attachments.is_empty()
    }

    /// Begin a brand-new message.
    pub fn begin_compose(&mut self, cx: &mut Context<Self>) {
        self.stash_draft(cx);
        self.model.reader = ReaderMode::Compose;
        self.model.focused = Some(Field::DraftTo);
        cx.notify();
    }

    /// Begin a reply (or reply-all) to the selected message.
    pub fn begin_reply(&mut self, all: bool, cx: &mut Context<Self>) {
        let Some(message) = self.model.current().cloned() else {
            return;
        };
        let own = self
            .model
            .account()
            .map(|account| account.email.clone())
            .unwrap_or_default();
        self.stash_draft(cx);

        let sender = format!("{} <{}>", message.sender(), message.row.from_addr);
        self.model.draft_to = sender;
        self.model.draft_cc.clear();
        self.model.draft_bcc.clear();

        if all {
            let (to, cc, _) = crate::ui::recipients::reply_recipients(&message, &own);
            self.model.draft_to = to;
            self.model.draft_cc = cc;
        }

        self.model.draft_subject = if message.row.subject.to_ascii_lowercase().starts_with("re:") {
            message.row.subject.clone()
        } else {
            format!("Re: {}", message.row.subject)
        };
        self.model.draft_in_reply_to = message.row.message_id.clone();
        if let Some(thread) = message.row.thread_id.clone() {
            self.model.draft_references = vec![thread];
        } else if let Some(id) = message.row.message_id.clone() {
            self.model.draft_references = vec![id];
        }
        self.model.reader = ReaderMode::Reply;
        self.model.focused = Some(Field::DraftBody);
        cx.notify();
    }

    /// Begin forwarding the selected message.
    pub fn begin_forward(&mut self, cx: &mut Context<Self>) {
        let Some(message) = self.model.current().cloned() else {
            return;
        };
        self.stash_draft(cx);
        self.model.draft_subject =
            if message.row.subject.to_ascii_lowercase().starts_with("fwd:") {
                message.row.subject.clone()
            } else {
                format!("Fwd: {}", message.row.subject)
            };
        self.model.draft_body = message
            .body
            .clone()
            .or_else(|| message.row.body_text.clone())
            .unwrap_or_default();
        self.model.reader = ReaderMode::Compose;
        self.model.focused = Some(Field::DraftTo);
        cx.notify();
    }

    /// Save the active draft to the local cache. Returns the draft row id.
    fn save_draft_local(&mut self) -> Option<i64> {
        if !self.draft_has_content() || self.model.demo {
            return self.model.draft_id;
        }
        let account_id = self.model.account().map(|account| account.id.clone())?;
        if self.model.draft_message_id.is_none() {
            self.model.draft_message_id = Some(new_message_id(&account_id));
        }
        let row = crate::db::DraftRow {
            id: self.model.draft_id.unwrap_or(0),
            account_id,
            message_id: self.model.draft_message_id.clone(),
            to_addrs: self.model.draft_to.clone(),
            cc_addrs: self.model.draft_cc.clone(),
            bcc_addrs: self.model.draft_bcc.clone(),
            subject: self.model.draft_subject.clone(),
            body: self.model.draft_body.clone(),
            in_reply_to: self.model.draft_in_reply_to.clone(),
            references: self.model.draft_references.join(" "),
            updated_at: unix_now(),
            remote: false,
        };
        let was_new = self.model.draft_id.is_none();
        let result = self.model.db.as_ref().map(|db| db.upsert_draft(&row));
        match result {
            Some(Ok(id)) => {
                self.model.draft_id = Some(id);
                self.persist_draft_attachments(id);
                self.model.draft_saved_at = Some(std::time::Instant::now());
                if was_new {
                    self.model.draft_count = self.model.draft_count.saturating_add(1);
                }
                Some(id)
            }
            Some(Err(error)) => {
                tracing::warn!("could not save draft: {error:#}");
                None
            }
            None => None,
        }
    }

    fn draft_signature(&self) -> String {
        let files: Vec<String> = self
            .compose_attachments
            .iter()
            .map(|item| item.path.to_string_lossy().into_owned())
            .collect();
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}",
            self.model.draft_to,
            self.model.draft_cc,
            self.model.draft_bcc,
            self.model.draft_subject,
            self.model.draft_body,
            self.model.draft_in_reply_to.clone().unwrap_or_default(),
            self.model.draft_references.join(" "),
            files.join(";")
        )
    }

    /// Save the draft locally on every keystroke, then debounce the upload.
    fn schedule_draft_save(&mut self, cx: &mut Context<Self>) {
        if self.model.demo
            || !matches!(self.model.reader, ReaderMode::Compose | ReaderMode::Reply)
        {
            return;
        }
        // Local writes are cheap, so nothing is lost if the app exits before
        // the server copy catches up.
        self.save_draft_local();
        self.draft_save_gen = self.draft_save_gen.wrapping_add(1);
        let generation = self.draft_save_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(900))
                .await;
            this.update(cx, |this, cx| {
                if this.draft_save_gen == generation {
                    this.flush_draft(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Persist the draft locally and mirror it into the IMAP Drafts folder.
    pub fn flush_draft(&mut self, cx: &mut Context<Self>) {
        if self.model.demo
            || !self.draft_has_content()
            || !matches!(self.model.reader, ReaderMode::Compose | ReaderMode::Reply)
        {
            return;
        }
        if self.save_draft_local().is_none() {
            return;
        }
        let signature = self.draft_signature();
        if self.draft_last_saved_sig.as_deref() == Some(signature.as_str()) {
            cx.notify();
            return;
        }
        self.draft_last_saved_sig = Some(signature);
        self.upload_draft(cx);
        cx.notify();
    }

    fn upload_draft(&mut self, cx: &mut Context<Self>) {
        if self.draft_uploading {
            self.draft_upload_pending = true;
            return;
        }
        let Some(account) = self.model.account().cloned() else {
            return;
        };
        let config = self.model.config.clone();
        let message_id = self.model.draft_message_id.clone();
        let draft_id = self.model.draft_id;
        let from = crate::mail::MailAddress {
            name: account.display_name.clone(),
            email: account.email.clone(),
        };
        let draft = crate::mail::Draft {
            from: from.clone(),
            to: parse_addresses(&self.model.draft_to),
            cc: parse_addresses(&self.model.draft_cc),
            bcc: parse_addresses(&self.model.draft_bcc),
            subject: self.model.draft_subject.clone(),
            body_text: self.model.draft_body.clone(),
            body_html: None,
            in_reply_to: self.model.draft_in_reply_to.clone(),
            references: self.model.draft_references.clone(),
            message_id: message_id.clone(),
            attachments: self.compose_attachments.clone(),
        };
        self.draft_uploading = true;
        let epoch = self.draft_upload_epoch;
        let cleanup_id = message_id.clone();

        let task = cx.background_executor().spawn(async move {
            let credential = crate::mail::auth::credential(&account, &config)?;
            let mut session = crate::mail::imap::connect(&account, &credential)?;
            let folder = crate::db::Db::open_default()
                .ok()
                .and_then(|db| db.folder_by_role(&account.id, "drafts").ok().flatten())
                .map(|folder| folder.name)
                .or_else(|| {
                    session
                        .folder_for_role(crate::mail::FolderRole::Drafts)
                        .ok()
                        .flatten()
                })
                .unwrap_or_else(|| "Drafts".to_string());
            // Replace any previous copy of this draft on the server.
            if let Some(id) = &message_id {
                if let Err(error) = session.delete_by_message_id(&folder, id) {
                    tracing::warn!("could not clear previous draft copy: {error:#}");
                }
            }
            let raw = crate::mail::smtp::build_message_raw(&from, &draft)?;
            session.append_raw(&folder, &raw, &[::imap::types::Flag::Draft])?;
            session.logout();
            Ok::<(), anyhow::Error>(())
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            if let Err(error) = &result {
                tracing::warn!("could not save draft to the server: {error:#}");
            }
            let uploaded = result.is_ok();
            this.update(cx, |this, cx| {
                this.draft_uploading = false;
                let stale = this.draft_upload_epoch != epoch;
                if uploaded && stale {
                    // The draft was sent or discarded while this upload was in
                    // flight, so remove the copy it just created.
                    if let (Some(account), Some(message_id)) =
                        (this.model.account().cloned(), cleanup_id.clone())
                    {
                        let config = this.model.config.clone();
                        cx.background_executor()
                            .spawn(async move {
                                if let Ok(credential) =
                                    crate::mail::auth::credential(&account, &config)
                                {
                                    if let Ok(mut session) =
                                        crate::mail::imap::connect(&account, &credential)
                                    {
                                        let folder = crate::db::Db::open_default()
                                            .ok()
                                            .and_then(|db| {
                                                db.folder_by_role(&account.id, "drafts")
                                                    .ok()
                                                    .flatten()
                                            })
                                            .map(|folder| folder.name)
                                            .or_else(|| {
                                                session
                                                    .folder_for_role(
                                                        crate::mail::FolderRole::Drafts,
                                                    )
                                                    .ok()
                                                    .flatten()
                                            })
                                            .unwrap_or_else(|| "Drafts".to_string());
                                        let _ = session
                                            .delete_by_message_id(&folder, &message_id);
                                        session.logout();
                                    }
                                }
                            })
                            .detach();
                    }
                } else if uploaded {
                    // The server now holds a copy, so it can reconcile
                    // deletions made from other clients.
                    if let (Some(id), Some(db)) = (draft_id, this.model.db.as_ref()) {
                        let _ = db.mark_draft_remote(id, true);
                    }
                } else {
                    // Retry on the next keystroke/close.
                    this.draft_last_saved_sig = None;
                }
                if this.draft_upload_pending {
                    this.draft_upload_pending = false;
                    this.flush_draft(cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Close the composer, keeping the draft.
    pub fn close_composer(&mut self, cx: &mut Context<Self>) {
        let was_composing = matches!(self.model.reader, ReaderMode::Compose | ReaderMode::Reply);
        if was_composing {
            if !self.draft_has_content() {
                // Clearing a draft and closing it throws the draft away.
                if self.model.draft_id.is_some() {
                    self.discard_draft(cx);
                    return;
                }
            } else {
                self.flush_draft(cx);
                self.toast("Draft saved", cx);
            }
        }
        self.model.reader = ReaderMode::Read;
        self.model.focused = None;
        self.model.reset_draft();
        self.compose_attachments.clear();
        if self.model.folder == "drafts" {
            self.reload_mail(cx);
        }
        cx.notify();
    }

    /// Throw away the draft being composed, locally and on the server.
    pub fn discard_draft(&mut self, cx: &mut Context<Self>) {
        // Cancel any debounced save and mark in-flight uploads as stale.
        self.draft_save_gen = self.draft_save_gen.wrapping_add(1);
        self.draft_upload_epoch = self.draft_upload_epoch.wrapping_add(1);
        let id = self.model.draft_id;
        let message_id = self.model.draft_message_id.clone();
        let account = self.model.account().cloned();
        let config = self.model.config.clone();

        if let Some(id) = id {
            if let Some(db) = self.model.db.as_ref() {
                let _ = db.delete_draft(id);
            }
            self.model.draft_count = self.model.draft_count.saturating_sub(1);
        }
        self.model.reset_draft();
        self.compose_attachments.clear();
        self.model.reader = ReaderMode::Read;
        self.model.focused = None;
        self.draft_last_saved_sig = None;

        if let (Some(account), Some(message_id)) = (account, message_id) {
            let task = cx.background_executor().spawn(async move {
                let credential = crate::mail::auth::credential(&account, &config)?;
                let mut session = crate::mail::imap::connect(&account, &credential)?;
                let folder = crate::db::Db::open_default()
                    .ok()
                    .and_then(|db| db.folder_by_role(&account.id, "drafts").ok().flatten())
                    .map(|folder| folder.name)
                    .or_else(|| {
                        session
                            .folder_for_role(crate::mail::FolderRole::Drafts)
                            .ok()
                            .flatten()
                    })
                    .unwrap_or_else(|| "Drafts".to_string());
                let _ = session.delete_by_message_id(&folder, &message_id);
                session.logout();
                Ok::<(), anyhow::Error>(())
            });
            cx.spawn(async move |this, cx| {
                if let Err(error) = task.await {
                    tracing::warn!("could not delete the server draft: {error:#}");
                }
                this.update(cx, |_, cx| cx.notify()).ok();
            })
            .detach();
        }

        if self.model.folder == "drafts" {
            self.reload_mail(cx);
        }
        self.toast("Draft discarded", cx);
    }

    /// Reopen a stored draft in the composer.
    pub fn resume_draft(&mut self, id: i64, cx: &mut Context<Self>) {
        let row = self
            .model
            .db
            .as_ref()
            .and_then(|db| db.draft(id).ok().flatten());
        let Some(row) = row else {
            return;
        };
        let reply = row.in_reply_to.is_some();
        self.compose_attachments = self
            .model
            .db
            .as_ref()
            .and_then(|db| db.draft_attachments(id).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|item| crate::mail::OutgoingAttachment {
                filename: item.filename,
                mime: item.mime,
                size: item.size,
                path: std::path::PathBuf::from(item.path),
            })
            .collect();
        self.model.adopt_draft(row);
        self.model.reader = if reply {
            ReaderMode::Reply
        } else {
            ReaderMode::Compose
        };
        self.model.focused = Some(Field::DraftBody);
        self.draft_last_saved_sig = Some(self.draft_signature());
        cx.notify();
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

        // Cancel any debounced save and mark in-flight uploads as stale.
        self.draft_save_gen = self.draft_save_gen.wrapping_add(1);
        self.draft_upload_epoch = self.draft_upload_epoch.wrapping_add(1);

        let config = self.model.config.clone();
        let message_id = self.model.draft_message_id.clone();
        let draft_id = self.model.draft_id;
        let draft = crate::mail::Draft {
            from: crate::mail::MailAddress {
                name: account.display_name.clone(),
                email: account.email.clone(),
            },
            to: parse_addresses(&self.model.draft_to),
            cc: parse_addresses(&self.model.draft_cc),
            bcc: parse_addresses(&self.model.draft_bcc),
            subject: self.model.draft_subject.clone(),
            body_text: self.model.draft_body.clone(),
            body_html: None,
            in_reply_to: self.model.draft_in_reply_to.clone(),
            references: self.model.draft_references.clone(),
            message_id: message_id.clone(),
            attachments: self.compose_attachments.clone(),
        };
        let from = crate::mail::MailAddress {
            name: account.display_name.clone(),
            email: account.email.clone(),
        };
        let task = cx.background_executor().spawn(async move {
            let credential = crate::mail::auth::credential(&account, &config)?;
            let secret = match &credential {
                crate::mail::imap::Credential::Password(value) => value.clone(),
                crate::mail::imap::Credential::OAuth(value) => value.clone(),
            };
            crate::mail::smtp::send(&account, &secret, &draft)?;

            // Save a copy into the IMAP Sent folder. Prefer the cached folder,
            // otherwise discover it from the server (handles localized names
            // like Spanish Gmail's "[Gmail]/Enviados").
            if let Ok(mut session) = crate::mail::imap::connect(&account, &credential) {
                let sent_folder = crate::db::Db::open_default()
                    .ok()
                    .and_then(|db| db.folder_by_role(&account.id, "sent").ok().flatten())
                    .map(|f| f.name)
                    .or_else(|| session.folder_for_role(crate::mail::FolderRole::Sent).ok().flatten())
                    .unwrap_or_else(|| "Sent".to_string());
                let raw = crate::mail::smtp::build_message_raw(&from, &draft)?;
                if let Err(error) = session.append(&sent_folder, &raw) {
                    tracing::warn!("could not append sent mail to {sent_folder}: {error:#}");
                }
                // Remove the draft copy now that the message has been sent.
                if let Some(id) = &message_id {
                    let drafts_folder = crate::db::Db::open_default()
                        .ok()
                        .and_then(|db| db.folder_by_role(&account.id, "drafts").ok().flatten())
                        .map(|f| f.name)
                        .or_else(|| {
                            session
                                .folder_for_role(crate::mail::FolderRole::Drafts)
                                .ok()
                                .flatten()
                        })
                        .unwrap_or_else(|| "Drafts".to_string());
                    if let Err(error) = session.delete_by_message_id(&drafts_folder, id) {
                        tracing::warn!("could not delete the sent draft: {error:#}");
                    }
                }
                session.logout();
            }

            if let Some(id) = draft_id {
                if let Ok(db) = crate::db::Db::open_default() {
                    let _ = db.delete_draft(id);
                }
            }
            Ok::<(), anyhow::Error>(())
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.toast("Sent", cx);
                        this.model.reader = ReaderMode::Read;
                        if draft_id.is_some() {
                            this.model.draft_count = this.model.draft_count.saturating_sub(1);
                        }
                        this.model.reset_draft();
                        this.compose_attachments.clear();
                        this.draft_last_saved_sig = None;
                        this.draft_save_gen = this.draft_save_gen.wrapping_add(1);
                        if this.model.folder == "drafts" {
                            this.reload_mail(cx);
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

    /// Apply an action to the current message on the server and in the cache,
    /// then update the in-memory model on the main thread.
    pub fn message_action(&mut self, action: MessageAction, cx: &mut Context<Self>) {
        if self.model.demo {
            self.toast("Demo mode — action not synced", cx);
            return;
        }
        if let Some(draft_id) = self.model.current().and_then(|message| message.draft_id) {
            match action {
                MessageAction::Trash => self.discard_draft(cx),
                _ => {
                    let _ = draft_id;
                    self.toast("Open the draft to edit or discard it", cx);
                }
            }
            return;
        }
        let Some(message) = self.model.current().cloned() else {
            return;
        };
        let Some(account) = self.model.account().cloned() else {
            self.toast("Connect an account first", cx);
            return;
        };
        let config = self.model.config.clone();
        let id = message.row.id;
        let uid = message.row.uid;
        let source_folder = message.row.folder.clone();

        // Destination is resolved inside the task: prefer the cache, then the
        // live server (handles localized folder names).
        let dest_role = match &action {
            MessageAction::Archive => Some(crate::mail::FolderRole::Archive),
            MessageAction::Trash => Some(crate::mail::FolderRole::Trash),
            _ => None,
        };
        let explicit_dest = match &action {
            MessageAction::MoveTo(name) => Some(name.clone()),
            _ => None,
        };

        let label = match &action {
            MessageAction::Archive => "Archived",
            MessageAction::Trash => "Moved to Trash",
            MessageAction::MoveTo(_) => "Moved",
            MessageAction::MarkUnread(true) => "Marked unread",
            MessageAction::MarkUnread(false) => "Marked read",
            MessageAction::Star(true) => "Starred",
            MessageAction::Star(false) => "Unstarred",
        }
        .to_string();
        let mark_unread = match &action {
            MessageAction::MarkUnread(v) => Some(*v),
            _ => None,
        };
        let set_star = match &action {
            MessageAction::Star(v) => Some(*v),
            _ => None,
        };

        let task = cx.background_executor().spawn(async move {
            let credential = crate::mail::auth::credential(&account, &config)?;
            let mut session = crate::mail::imap::connect(&account, &credential)?;
            let db = crate::db::Db::open_default().ok();
            let result = match &action {
                MessageAction::MarkUnread(unread) => {
                    if *unread {
                        session.mark_unseen(&source_folder, uid)?;
                    } else {
                        session.mark_seen(&source_folder, uid)?;
                    }
                    if let Some(db) = &db {
                        let flags = db
                            .message(id)
                            .ok()
                            .flatten()
                            .map(|row| row.flags)
                            .unwrap_or_default();
                        let flags = if *unread {
                            flags.replace("\\Seen", "").split_whitespace()
                                .collect::<Vec<_>>().join(" ")
                        } else if flags.contains("\\Seen") {
                            flags
                        } else {
                            format!("{} \\Seen", flags).trim().to_string()
                        };
                        let _ = db.set_flags(id, &flags);
                    }
                    Ok::<(), anyhow::Error>(())
                }
                MessageAction::Star(flagged) => {
                    session.mark_flagged(&source_folder, uid, *flagged)?;
                    if let Some(db) = &db {
                        let flags = db
                            .message(id)
                            .ok()
                            .flatten()
                            .map(|row| row.flags)
                            .unwrap_or_default();
                        let flags = if *flagged {
                            if flags.contains("\\Flagged") {
                                flags
                            } else {
                                format!("{} \\Flagged", flags).trim().to_string()
                            }
                        } else {
                            flags.replace("\\Flagged", "").split_whitespace()
                                .collect::<Vec<_>>().join(" ")
                        };
                        let _ = db.set_flags(id, &flags);
                    }
                    Ok(())
                }
                MessageAction::Archive | MessageAction::Trash | MessageAction::MoveTo(_) => {
                    let dest = if let Some(name) = &explicit_dest {
                        name.clone()
                    } else if let Some(role) = dest_role {
                        // Prefer cached name, then discover from the server.
                        let cached = crate::db::Db::open_default().ok()
                            .and_then(|db| db.folder_by_role(&account.id, role.as_str()).ok().flatten())
                            .map(|f| f.name);
                        cached
                            .or_else(|| session.folder_for_role(role).ok().flatten())
                            .ok_or_else(|| anyhow::anyhow!("destination folder not found on server"))?
                    } else {
                        return Err(anyhow::anyhow!("no destination folder"));
                    };
                    session.move_to(&source_folder, uid, &dest)?;
                    if let Some(db) = &db {
                        let _ = db.delete_message(id);
                    }
                    Ok(())
                }
            };
            session.logout();
            result
        });

        cx.spawn(async move |this, cx| {
            match task.await {
                Ok(()) => {
                    this.update(cx, |this, cx| {
                        match mark_unread {
                            Some(unread) => this.model.set_message_unread(id, unread),
                            None => match set_star {
                                Some(flagged) => this.model.set_message_star(id, flagged),
                                None => this.model.remove_message(id),
                            },
                        }
                        cx.notify();
                        this.toast(label, cx);
                    })
                    .ok();
                }
                Err(error) => {
                    this.update(cx, |this, cx| {
                        this.toast(format!("Action failed: {error:#}"), cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Star/unstar the current message based on its current state.
    pub fn star_current(&mut self, cx: &mut Context<Self>) {
        let flagged = self
            .model
            .current()
            .map(|m| m.row.flags.contains("\\Flagged"))
            .unwrap_or(false);
        self.message_action(MessageAction::Star(!flagged), cx);
    }
}

/// A unique, stable Message-ID for a draft created on this device.
fn new_message_id(account_id: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let tag: String = account_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(12)
        .collect();
    format!("{nanos}.{tag}@clean.local")
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

pub fn parse_addresses(value: &str) -> Vec<crate::mail::MailAddress> {    value
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // viewport_size is in device pixels; panes are sized in logical px.
        let window_width = window.viewport_size().width / window.scale_factor();
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
                this.begin_compose(cx);
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, _, cx| this.open_settings(cx)))
            .on_action(cx.listener(|this, _: &SyncNow, _, cx| this.sync_now(cx)))
            .on_action(cx.listener(|this, _: &ToggleView, _, cx| this.toggle_view(cx)))
            .on_action(cx.listener(|this, _: &ArchiveMessage, _, cx| {
                this.message_action(MessageAction::Archive, cx)
            }))
            .on_action(cx.listener(|this, _: &DeleteMessage, _, cx| {
                this.message_action(MessageAction::Trash, cx)
            }))
            .on_action(cx.listener(|this, _: &NextMessage, _, cx| this.step_selection(1, cx)))
            .on_action(cx.listener(|this, _: &PreviousMessage, _, cx| this.step_selection(-1, cx)))
            .on_action(cx.listener(|this, _: &CloseOverlay, _, cx| {
                this.model.show_settings = false;
                this.model.reader = ReaderMode::Read;
                cx.notify();
            }))
            .child(match self.model.screen {
                Screen::Setup => setup::setup(self, cx).into_any_element(),
                Screen::Mail => shell::shell(self, cx, window_width).into_any_element(),
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

