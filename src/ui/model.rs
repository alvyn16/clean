use std::collections::HashSet;

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike};

use crate::config::{AccountConfig, Config};
use crate::db::{Db, FolderRow, MessageRow};
use crate::theme::{Accent, OmarchyColors, Palette, ThemeMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Setup,
    Mail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupStep {
    Appearance,
    Account,
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailView {
    Mail,
    Calendar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderMode {
    Read,
    Reply,
    Compose,
}

#[derive(Clone, Debug)]
pub struct EventDraft {
    pub title: String,
    pub day: String,
    pub start: String,
    pub end: String,
    pub emoji: String,
    pub color: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Search,
    Email,
    Password,
    DraftTo,
    DraftSubject,
    DraftBody,
    EventTitle,
    EventDate,
    EventStart,
    EventEnd,
}

#[derive(Clone, Debug)]
pub struct UiMessage {
    pub row: MessageRow,
    pub labels: Vec<String>,
    pub unread: bool,
    pub pinned: bool,
    pub body: Option<String>,
}

impl UiMessage {
    pub fn from_row(row: MessageRow) -> Self {
        let unread = !row.flags.contains("\\Seen");
        Self {
            unread,
            row,
            labels: Vec::new(),
            pinned: false,
            body: None,
        }
    }

    pub fn sender(&self) -> &str {
        if self.row.from_name.is_empty() {
            &self.row.from_addr
        } else {
            &self.row.from_name
        }
    }

    pub fn initial(&self) -> String {
        self.sender()
            .chars()
            .find(|c| c.is_alphanumeric())
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "•".into())
    }

    pub fn preview(&self) -> String {
        if !self.row.snippet.trim().is_empty() {
            self.row.snippet.clone()
        } else if let Some(body) = self.body.as_deref() {
            body.split_whitespace().collect::<Vec<_>>().join(" ")
        } else {
            String::new()
        }
    }

    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return true;
        }
        let haystack = format!(
            "{} {} {} {}",
            self.row.subject,
            self.row.from_name,
            self.row.from_addr,
            self.preview()
        )
        .to_ascii_lowercase();
        query.split_whitespace().all(|term| haystack.contains(term))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Label {
    pub name: String,
    pub count: u32,
}

#[derive(Clone, Debug)]
pub struct EventColor {
    pub name: &'static str,
    pub light: u32,
    pub dark: u32,
}

impl EventColor {
    pub fn rgb(&self, dark_mode: bool) -> u32 {
        if dark_mode {
            self.dark
        } else {
            self.light
        }
    }
}

pub const EVENT_COLORS: [EventColor; 6] = [
    EventColor {
        name: "ocean",
        light: 0x2F6F9F,
        dark: 0x7FBBB3,
    },
    EventColor {
        name: "amber",
        light: 0xB45309,
        dark: 0xDBBC7F,
    },
    EventColor {
        name: "violet",
        light: 0x6D4AA6,
        dark: 0xB4A0E0,
    },
    EventColor {
        name: "rose",
        light: 0xB04A72,
        dark: 0xE6A0B8,
    },
    EventColor {
        name: "moss",
        light: 0x4F7A45,
        dark: 0xA7C080,
    },
    EventColor {
        name: "clay",
        light: 0xA6533A,
        dark: 0xE6987A,
    },
];

#[derive(Clone, Debug)]
pub struct CalendarEvent {
    pub id: usize,
    pub day: NaiveDate,
    pub start: u32,
    pub end: u32,
    pub title: String,
    pub emoji: String,
    pub color: usize,
}

impl CalendarEvent {
    pub fn start_time(&self) -> NaiveDateTime {
        self.day
            .and_hms_opt(self.start / 60, self.start % 60, 0)
            .unwrap_or_default()
    }

    pub fn time_label(&self) -> String {
        let start = format_clock(self.start);
        let end = format_clock(self.end);
        format!("{start} – {end}")
    }
}

pub struct Model {
    pub config: Config,
    pub db: Option<Db>,
    pub palette: Palette,
    pub omarchy: Option<OmarchyColors>,
    pub screen: Screen,
    pub setup_step: SetupStep,
    pub view: MailView,
    pub folder: String,
    pub selected: Option<usize>,
    pub messages: Vec<UiMessage>,
    pub folders: Vec<FolderRow>,
    pub labels: Vec<Label>,
    pub events: Vec<CalendarEvent>,
    pub search: String,
    pub reader: ReaderMode,
    pub show_settings: bool,
    pub demo: bool,
    pub syncing: bool,
    pub sync_note: Option<String>,
    pub toast: Option<(String, std::time::Instant)>,
    pub today: NaiveDate,
    pub week_start: NaiveDate,
    pub draft_to: String,
    pub draft_subject: String,
    pub draft_body: String,
    pub sign_in: SignInState,
    pub need_onboarding: bool,
    pub focused: Option<Field>,
    pub show_event_sheet: bool,
    pub show_account_manager: bool,
    pub event_draft: EventDraft,
    pub show_account_menu: bool,
    pub active_account: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SignInState {
    pub provider: Option<crate::config::Provider>,
    pub email: String,
    pub password: String,
    pub busy: bool,
    pub error: Option<String>,
}

impl Default for SignInState {
    fn default() -> Self {
        Self {
            // The account screen shows Google as the selected provider by
            // default, so the state must match it — otherwise the first
            // "Continue with Google" click hits start_sign_in's early return.
            provider: Some(crate::config::Provider::Google),
            email: String::new(),
            password: String::new(),
            busy: false,
            error: None,
        }
    }
}

impl Model {
    pub fn new(force_setup: bool) -> Self {
        let config = Config::load().unwrap_or_default();
        let need_onboarding = force_setup || !config.onboarding.complete;
        let db = Db::open_default().ok();

        let today = Local::now().date_naive();
        let mut model = Self {
            palette: Palette::black(crate::theme::Rgba::hex(0x7fbbb3)),
            omarchy: crate::theme::omarchy::read_live(),
            config,
            db,
            screen: if need_onboarding {
                Screen::Setup
            } else {
                Screen::Mail
            },
            setup_step: SetupStep::Appearance,
            view: MailView::Mail,
            folder: "inbox".into(),
            selected: None,
            messages: Vec::new(),
            folders: Vec::new(),
            labels: Vec::new(),
            events: Vec::new(),
            search: String::new(),
            reader: ReaderMode::Read,
            show_settings: false,
            demo: false,
            syncing: false,
            sync_note: None,
            toast: None,
            today,
            week_start: week_start(today),
            draft_to: String::new(),
            draft_subject: String::new(),
            draft_body: String::new(),
            sign_in: SignInState::default(),
            need_onboarding,
            focused: None,
            show_event_sheet: false,
            show_account_manager: false,
            show_account_menu: false,
            active_account: None,
            event_draft: EventDraft {
                title: String::new(),
                day: today.format("%Y-%m-%d").to_string(),
                start: "09:00".into(),
                end: "10:00".into(),
                emoji: "📅".into(),
                color: 0,
            },
        };
        model.load_saved_credentials();

        model.rebuild_theme();
        if model.config.accounts.is_empty() {
            model.load_demo();
        } else {
            model.reload();
        }
        model.reload_events();
        model
    }

    pub fn rebuild_theme(&mut self) {
        let mode = self.config.appearance.mode;
        let accent = self.config.appearance.accent();
        let omarchy_blue = self.omarchy.as_ref().map(OmarchyColors::blue);
        let resolved = accent.resolve(mode, omarchy_blue);

        self.palette = match mode {
            ThemeMode::White => Palette::white(resolved),
            ThemeMode::Black => Palette::black(resolved),
            ThemeMode::Omarchy => match self.omarchy.as_ref() {
                Some(colors) => colors.palette(resolved),
                None => Palette::black(resolved),
            },
        };
    }

    pub fn set_mode(&mut self, mode: ThemeMode) {
        self.config.appearance.mode = mode;
        self.rebuild_theme();
    }

    pub fn set_accent(&mut self, accent: Accent) {
        self.config.appearance.accent = accent.as_config_str();
        self.rebuild_theme();
    }

    pub fn accent(&self) -> Accent {
        self.config.appearance.accent()
    }

    /// Pre-fill Apple/Proton credentials from the keyring so signing in is one click.
    pub fn load_saved_credentials(&mut self) {
        for account in &self.config.accounts {
            if !matches!(
                account.provider,
                crate::config::Provider::Apple | crate::config::Provider::Proton
            ) {
                continue;
            }
            if let Ok(Some(password)) = crate::secrets::get(&account.id, "password") {
                self.sign_in.provider = Some(account.provider);
                self.sign_in.email = account.email.clone();
                self.sign_in.password = password;
                return;
            }
        }
    }

    /// Load persisted events; demo events fill the rail when the cache is empty.
    pub fn reload_events(&mut self) {
        let stored = self
            .db
            .as_ref()
            .and_then(|db| db.events().ok())
            .unwrap_or_default();
        // Demo events only exist to populate the calendar in demo mode.
        // A connected account starts with a genuinely empty calendar.
        let mut events = if self.demo {
            demo_events(self.today)
        } else {
            Vec::new()
        };
        for row in stored {
            events.push(CalendarEvent {
                id: 10_000 + row.id as usize,
                day: NaiveDate::parse_from_str(&row.day, "%Y-%m-%d").unwrap_or(self.today),
                start: row.start_min.max(0) as u32,
                end: row.end_min.max(0) as u32,
                title: row.title,
                emoji: row.emoji,
                color: row.color.max(0) as usize,
            });
        }
        events.sort_by_key(|event| event.start_time());
        events.dedup_by_key(|event| event.id);
        self.events = events;
    }

    pub fn open_event_sheet(
        &mut self,
        title: Option<String>,
        day: NaiveDate,
        start: u32,
        end: u32,
    ) {
        self.event_draft = EventDraft {
            title: title.unwrap_or_default(),
            day: day.format("%Y-%m-%d").to_string(),
            start: format!("{:02}:{:02}", start / 60, start % 60),
            end: format!("{:02}:{:02}", end / 60, end % 60),
            emoji: "📅".into(),
            color: 0,
        };
        self.show_event_sheet = true;
        self.focused = Some(Field::EventTitle);
    }

    /// Persist the event draft and refresh every calendar surface.
    pub fn add_event(&mut self) -> bool {
        let draft = self.event_draft.clone();
        if draft.title.trim().is_empty() {
            return false;
        }
        let start = parse_hhmm(&draft.start).unwrap_or(9 * 60);
        let mut end = parse_hhmm(&draft.end).unwrap_or(start + 60);
        if end <= start {
            end = start + 60;
        }
        let day = NaiveDate::parse_from_str(&draft.day, "%Y-%m-%d").unwrap_or(self.today);
        let row = crate::db::EventRow {
            id: 0,
            title: draft.title.trim().to_string(),
            day: day.format("%Y-%m-%d").to_string(),
            start_min: start as i32,
            end_min: end as i32,
            emoji: if draft.emoji.trim().is_empty() {
                "📅".into()
            } else {
                draft.emoji
            },
            color: draft.color as i32,
        };
        let Some(db) = self.db.as_ref() else {
            return false;
        };
        if db.add_event(&row).is_err() {
            return false;
        }
        self.show_event_sheet = false;
        self.focused = None;
        self.reload_events();
        true
    }

    pub fn save(&mut self) {
        if let Err(error) = self.config.save() {
            self.show_toast(format!("Could not save settings: {error}"));
        }
    }

    pub fn show_toast(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), std::time::Instant::now()));
    }

    pub fn account(&self) -> Option<&AccountConfig> {
        self.config
            .accounts
            .iter()
            .find(|account| Some(&account.id) == self.active_account.as_ref())
            .or_else(|| self.config.accounts.first())
    }

    pub fn set_active_account(&mut self, id: String) {
        self.active_account = Some(id);
        self.reader = ReaderMode::Read;
        self.selected = None;
        self.search.clear();
        if self.config.accounts.len() > 1 {
            self.reload();
        }
    }

    pub fn load_demo(&mut self) {
        self.demo = true;
        self.folders = demo_folders();
        self.labels = demo_labels();
        self.messages = demo_messages(self.today);
        self.selected = self
            .messages
            .iter()
            .position(|message| message.row.subject.contains("morning walk"));
        self.sync_note = Some("Demo mail".into());
    }

    /// Read folders and inbox-level messages for the active account from the local cache.
    pub fn reload(&mut self) {
        let account_id = self.account().map(|account| account.id.clone());
        let Some(account_id) = account_id else {
            self.load_demo();
            return;
        };
        let Some(db) = self.db.as_ref() else {
            self.load_demo();
            return;
        };

        let folders = db.folders(&account_id).unwrap_or_default();
        if folders.is_empty() {
            // This account has no cached folders yet — show an empty inbox for it,
            // not another account's messages.
            self.demo = false;
            self.folders = Vec::new();
            self.messages = Vec::new();
            self.labels = Vec::new();
            self.selected = None;
            self.sync_note = Some("No mail cached for this account".into());
            return;
        }
        self.demo = false;
        self.folders = folders;

        let folder_name = self
            .folders
            .iter()
            .find(|folder| folder.role == self.folder)
            .map(|folder| folder.name.clone())
            .unwrap_or_else(|| "INBOX".into());

        let rows = db
            .list_messages(&account_id, &folder_name, 200, 0)
            .unwrap_or_default();

        self.messages = rows.into_iter().map(UiMessage::from_row).collect();
        self.labels = self
            .folders
            .iter()
            .filter(|folder| folder.role == "custom")
            .map(|folder| Label {
                name: folder.name.clone(),
                count: folder.unread,
            })
            .collect();

        if self
            .selected
            .is_none_or(|index| index >= self.messages.len())
        {
            self.selected = if self.messages.is_empty() {
                None
            } else {
                Some(0)
            };
        }
        self.sync_note = None;
    }

    pub fn select_folder(&mut self, folder: String) {
        self.folder = folder;
        self.search.clear();
        self.selected = None;
        self.reader = ReaderMode::Read;
        if self.demo {
            // Demo folders all show the same inbox; remap sensibly.
            self.selected = if self.messages.is_empty() {
                None
            } else {
                Some(0)
            };
        } else {
            self.reload();
        }
    }

    pub fn select_message(&mut self, index: usize) {
        self.selected = Some(index);
        self.reader = ReaderMode::Read;

        if self.demo {
            return;
        }

        let Some(db) = self.db.as_ref() else { return };
        let Some(message) = self.messages.get(index) else {
            return;
        };
        if message.body.is_some() {
            return;
        }
        if let Ok(Some(row)) = db.message(message.row.id) {
            if row.body_text.is_some() || row.body_html.is_some() {
                self.messages[index].body = row.body_text;
            }
        }
    }

    /// Mark a message read in the local model as soon as it is opened.
    pub fn mark_seen(&mut self, index: usize) {
        let folder_name = {
            let Some(message) = self.messages.get_mut(index) else {
                return;
            };
            if !message.unread {
                return;
            }
            message.unread = false;
            if !message.row.flags.contains("\\Seen") {
                message.row.flags = format!("{} \\Seen", message.row.flags).trim().to_string();
            }
            message.row.folder.clone()
        };

        if let Some(folder) = self
            .folders
            .iter_mut()
            .find(|folder| folder.name == folder_name || folder.role == folder_name)
        {
            folder.unread = folder.unread.saturating_sub(1);
        }
    }

    /// Store a lazily fetched body in the visible message and its cache row.
    pub fn set_body(&mut self, index: usize, text: String, html: Option<String>) {
        let Some(message) = self.messages.get_mut(index) else {
            return;
        };
        message.body = Some(text.clone());
        message.row.body_text = Some(text);
        message.row.body_html = html;
    }

    /// Replace the visible list with full-text search results.
    pub fn set_search_results(&mut self, rows: Vec<crate::db::MessageRow>) {
        self.messages = rows.into_iter().map(UiMessage::from_row).collect();
        self.selected = if self.messages.is_empty() {
            None
        } else {
            Some(0)
        };
        self.reader = ReaderMode::Read;
    }

    pub fn current(&self) -> Option<&UiMessage> {
        self.selected.and_then(|index| self.messages.get(index))
    }

    pub fn visible_messages(&self) -> Vec<&UiMessage> {
        self.messages
            .iter()
            .filter(|message| message.matches(&self.search))
            .collect()
    }

    pub fn unread_total(&self) -> u32 {
        self.folders.iter().map(|folder| folder.unread).sum()
    }

    pub fn folder_unread(&self, role: &str) -> u32 {
        if self.demo {
            return match role {
                "inbox" => 5,
                "spam" => 1,
                _ => 0,
            };
        }
        self.folders
            .iter()
            .find(|folder| folder.role == role)
            .map(|folder| folder.unread)
            .unwrap_or(0)
    }

    pub fn upcoming(&self) -> Vec<&CalendarEvent> {
        let now = Local::now();
        let mut events: Vec<&CalendarEvent> = self
            .events
            .iter()
            .filter(|event| event.start_time() >= now.naive_local())
            .collect();
        events.sort_by_key(|event| event.start_time());
        events.truncate(12);
        events
    }

    pub fn week_events(&self) -> Vec<&CalendarEvent> {
        let end = self.week_start + Duration::days(7);
        self.events
            .iter()
            .filter(|event| event.day >= self.week_start && event.day < end)
            .collect()
    }

    pub fn day_label(&self, day: NaiveDate) -> String {
        if day == self.today {
            "Today".into()
        } else if day == self.today + Duration::days(1) {
            "Tomorrow".into()
        } else {
            day.format("%A %-d %B").to_string()
        }
    }
}

pub fn week_start(day: NaiveDate) -> NaiveDate {
    day - Duration::days(day.weekday().num_days_from_sunday() as i64)
}

pub fn parse_hhmm(value: &str) -> Option<u32> {
    let (hour, minute) = value.trim().split_once(':')?;
    let hour: u32 = hour.trim().parse().ok()?;
    let minute: u32 = minute.trim().parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(hour * 60 + minute)
}

pub fn format_clock(minutes: u32) -> String {
    let hour = (minutes / 60) % 24;
    let minute = minutes % 60;
    let hour12 = if hour % 12 == 0 { 12 } else { hour % 12 };
    let suffix = if hour < 12 { "am" } else { "pm" };
    if minute == 0 {
        format!("{hour12} {suffix}")
    } else {
        format!("{hour12}:{minute:02} {suffix}")
    }
}

pub fn format_list_time(timestamp: i64, now: NaiveDate) -> String {
    let datetime = Local
        .timestamp_opt(timestamp, 0)
        .single()
        .unwrap_or_else(|| Local.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap());
    let date = datetime.date_naive();
    if date == now {
        datetime.format("%-I:%M %p").to_string()
    } else if date == now - Duration::days(1) {
        "Yesterday".into()
    } else if (now - date).num_days() < 7 && date < now {
        date.format("%a").to_string()
    } else {
        date.format("%b %-d").to_string()
    }
}

pub fn format_reader_time(timestamp: i64) -> String {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|datetime| datetime.format("%-I:%M %p").to_string())
        .unwrap_or_default()
}

fn demo_folders() -> Vec<FolderRow> {
    [
        ("inbox", "Inbox", 5),
        ("archive", "All Mail", 0),
        ("sent", "Sent", 0),
        ("drafts", "Drafts", 0),
        ("spam", "Spam", 1),
        ("trash", "Trash", 0),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (role, name, unread))| FolderRow {
        account_id: "demo".into(),
        name: name.into(),
        role: role.into(),
        delimiter: Some("/".into()),
        uid_validity: 1 + index as u32,
        uid_next: 100,
        total: 28,
        unread,
    })
    .collect()
}

fn demo_labels() -> Vec<Label> {
    [
        ("Work", 3),
        ("Friends", 2),
        ("Receipts", 1),
        ("Travel", 2),
        ("Reading", 1),
    ]
    .into_iter()
    .map(|(name, count)| Label {
        name: name.into(),
        count,
    })
    .collect()
}

fn demo_messages(today: NaiveDate) -> Vec<UiMessage> {
    let at = |hour: u32, minute: u32| {
        today
            .and_hms_opt(hour, minute, 0)
            .and_then(|value| value.and_local_timezone(Local).single())
            .map(|value| value.timestamp())
            .unwrap_or(0)
    };
    let yesterday = at(17, 20) - 86_400;

    let mut messages = vec![
        demo_message(
            1,
            "Maya Chen",
            "maya@example.com",
            "Biscuit's Barkday party 🎂",
            "Saturday at the park, 2 PM. Pup cake is sorted — just bring a squeaky toy.",
            at(10, 39),
            &["Friends", "Biscuit"],
            true,
            "Hi!\n\nIt's official — Biscuit turns three and we're throwing him a Barkday.\n\nSaturday at 2:00 PM, by the big oak at the dog park.\n\nThe bakery is making a peanut-butter pup cake with his name on it. Bring a squeaky toy (or two — he will steal one).\n\nCan't wait,\nMaya 🐾",
        ),
        demo_message(
            2,
            "Pawntry",
            "orders@pawntry.example",
            "Your order is on its way 📦",
            "Salmon bites, a squeaky duck and a new ball. Arriving Thursday.",
            at(9, 51),
            &["Receipts"],
            false,
            "Your order is on its way.\n\nSalmon bites, a squeaky duck and a new ball — arriving Thursday between 9 and 11.\n\nTrack your parcel from the order page.",
        ),
        demo_message(
            3,
            "Paws & Pines Vet",
            "hello@pawsandpines.example",
            "Checkup reminder for Biscuit",
            "Tuesday, November 10 at 10:30 AM with Dr. Rowan.",
            at(8, 33),
            &["Health"],
            true,
            "A little reminder that Biscuit's yearly checkup is coming up.\n\nTuesday at 10:30 AM with Dr. Rowan. He's due for his booster, and yes — the treat jar is full.",
        ),
        demo_message(
            4,
            "Tidewave Music",
            "hello@tidewave.example",
            "Your Dog Walk Mix is ready",
            "40 minutes of good-boy energy for the long way home.",
            at(6, 57),
            &["Reading"],
            false,
            "Your Dog Walk Mix is ready.\n\nForty minutes of good-boy energy for the long way home. We added three new tracks you haven't heard yet.",
        ),
        demo_message(
            5,
            "Cedar Account",
            "security@cedar.example",
            "Security check: new sign-in on Mac",
            "Your Cedar account was signed in on a new device. Was this you?",
            at(4, 57),
            &[],
            false,
            "Your Cedar account was signed in on a new device.\n\nDevice: MacBook Pro\nLocation: Amsterdam, Netherlands\n\nIf this was you, no action is needed. If not, change your password and sign out of other devices.",
        ),
        demo_message(
            6,
            "Dog Park Crew",
            "crew@dogpark.example",
            "Saturday morning walk?",
            "Canyon loop at 8, coffee after. Biscuit leads, as always.",
            at(1, 57),
            &["Friends"],
            true,
            "Hi!\n\nCanyon loop at 8, coffee after.\n\nBiscuit leads, as always. Everyone else follows the pug.\n\n— The crew",
        ),
        demo_message(
            7,
            "Harbor Stays",
            "stay@harbor.example",
            "Dog-friendly stays for your weekend",
            "Cabins with fenced yards, two hours from the city.",
            yesterday,
            &["Travel"],
            false,
            "Cabins with fenced yards, two hours from the city.\n\nA few of our favourites are still open for the weekend — wood stoves, long trails, and a welcome biscuit on arrival.",
        ),
        demo_message(
            8,
            "Sunny Paws Grooming",
            "hello@sunnypaws.example",
            "Biscuit's spa day",
            "He's booked for Thursday at 11. Nail trim and a bath.",
            yesterday - 3_600,
            &["Biscuit"],
            false,
            "Biscuit is booked for Thursday at 11.\n\nNail trim, a bath, and (if he sits still) the good towel. Pick-up around noon.",
        ),
    ];

    messages[5].pinned = false;
    messages
}

#[allow(clippy::too_many_arguments)]
fn demo_message(
    id: i64,
    name: &str,
    email: &str,
    subject: &str,
    snippet: &str,
    date: i64,
    labels: &[&str],
    unread: bool,
    body: &str,
) -> UiMessage {
    UiMessage {
        row: MessageRow {
            id,
            account_id: "demo".into(),
            folder: "INBOX".into(),
            uid: id as u32,
            message_id: Some(format!("<{id}@clean.local>")),
            thread_id: Some(format!("<{id}@clean.local>")),
            subject: subject.into(),
            from_name: name.into(),
            from_addr: email.into(),
            to_addrs: "you@clean.local".into(),
            date,
            snippet: snippet.into(),
            flags: if unread { String::new() } else { "\\Seen".into() },
            has_attachments: false,
            body_text: Some(body.into()),
            body_html: None,
        },
        labels: labels.iter().map(|label| label.to_string()).collect(),
        unread,
        pinned: false,
        body: Some(body.into()),
    }
}

fn demo_events(today: NaiveDate) -> Vec<CalendarEvent> {
    let monday = week_start(today);
    let event = |id: usize, day: NaiveDate, start: u32, end: u32, title: &str, emoji: &str, color: usize| {
        CalendarEvent {
            id,
            day,
            start,
            end,
            title: title.into(),
            emoji: emoji.into(),
            color,
        }
    };

    let mut events = vec![
        event(1, today, 7 * 60, 8 * 60, "Nap (non-negotiable)", "💤", 0),
        event(2, today, 8 * 60, 8 * 60 + 30, "Puppuccino run", "☕", 1),
        event(3, today, 12 * 60, 13 * 60, "Zooms at the beach", "🌊", 0),
        event(4, today + Duration::days(1), 10 * 60, 11 * 60, "Playdate with Waffles", "🐶", 1),
        event(5, today + Duration::days(1), 16 * 60, 17 * 60, "Squeaky toy shopping", "🧸", 1),
        event(6, today + Duration::days(2), 11 * 60, 11 * 60 + 30, "Nail trim (he'll be dramatic)", "💅", 3),
        event(7, today + Duration::days(2), 18 * 60, 19 * 60, "Bath day — hide the towels", "🛁", 0),
        event(8, today + Duration::days(3), 9 * 60, 10 * 60, "Agility class", "🏅", 2),
        event(9, today + Duration::days(3), 17 * 60, 17 * 60 + 30, "Pick up the pup cake", "🎂", 1),
        event(10, today + Duration::days(4), 14 * 60, 17 * 60, "Biscuit's Barkday party", "🎉", 5),
        event(11, today + Duration::days(5), 8 * 60, 9 * 60 + 30, "Sunrise walk", "🌄", 4),
        event(12, today + Duration::days(6), 10 * 60, 11 * 60, "Slow morning", "🍃", 4),
        event(13, monday - Duration::days(1), 15 * 60, 16 * 60, "Long walk by the river", "🌿", 4),
        event(14, monday + Duration::days(2), 13 * 60, 14 * 60, "Groomer call", "📞", 2),
    ];

    // Give the demo week a shaped look: one early event and one late event.
    events.sort_by_key(|event| event.start_time());
    events
}

pub fn label_color(name: &str, accent: crate::theme::Rgba, dark: bool) -> crate::theme::Rgba {
    let palette = if dark {
        [0xA7C080, 0xDBBC7F, 0xB4A0E0, 0xE6A0B8, 0x7FB4D4, 0xE6987A]
    } else {
        [0x4F7A45, 0xB45309, 0x6D4AA6, 0xB04A72, 0x2F6F9F, 0xA6533A]
    };
    let hash = name
        .bytes()
        .fold(2166136261u32, |hash, byte| {
            hash.wrapping_mul(16777619) ^ byte as u32
        }) as usize;
    let _ = accent;
    crate::theme::Rgba::hex(palette[hash % palette.len()])
}

pub fn sender_color(name: &str, dark: bool) -> crate::theme::Rgba {
    label_color(name, crate::theme::Rgba::hex(0x7fbbb3), dark)
}

pub fn event_color(index: usize, dark: bool) -> crate::theme::Rgba {
    crate::theme::Rgba::hex(EVENT_COLORS[index % EVENT_COLORS.len()].rgb(dark))
}

/// Normalize plain text bodies into readable paragraphs.
pub fn paragraphs(body: &str) -> Vec<String> {
    body.replace("\r\n", "\n")
        .replace('\r', "\n")
        .split("\n\n")
        .map(|paragraph| paragraph.trim().to_string())
        .filter(|paragraph| !paragraph.is_empty())
        .collect()
}

pub fn unique_labels(messages: &[UiMessage]) -> HashSet<String> {
    messages
        .iter()
        .flat_map(|message| message.labels.iter().cloned())
        .collect()
}

pub fn now_minutes() -> u32 {
    let now = Local::now();
    now.hour() * 60 + now.minute()
}

pub fn minutes_from_time(time: NaiveTime) -> u32 {
    time.hour() * 60 + time.minute()
}
