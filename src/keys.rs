//! Editable key bindings: parsing, display and matching.

use std::collections::BTreeMap;

/// A single key chord, e.g. Ctrl+N or Shift+#.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chord {
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
}

impl Chord {
    pub fn matches(
        &self,
        key: &str,
        ctrl: bool,
        alt: bool,
        shift: bool,
        super_key: bool,
    ) -> bool {
        self.key.eq_ignore_ascii_case(key)
            && self.ctrl == ctrl
            && self.alt == alt
            && self.shift == shift
            && self.super_key == super_key
    }

    /// Canonical serialised form, e.g. `ctrl+n`.
    pub fn encode(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl { parts.push("ctrl"); }
        if self.alt { parts.push("alt"); }
        if self.shift { parts.push("shift"); }
        if self.super_key { parts.push("super"); }
        parts.push(&self.key);
        parts.join("+")
    }

    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl { parts.push("Ctrl".to_string()); }
        if self.alt { parts.push("Alt".to_string()); }
        if self.shift { parts.push("Shift".to_string()); }
        if self.super_key { parts.push("Super".to_string()); }
        parts.push(key_label(&self.key));
        parts.join(" ")
    }
}

/// Parse `ctrl+n`, `shift+#`, etc. Returns None for empty or modifier-only
/// chords, which are not useful bindings.
pub fn parse_chord(value: &str) -> Option<Chord> {
    let mut chord = Chord {
        key: String::new(),
        ctrl: false,
        alt: false,
        shift: false,
        super_key: false,
    };
    let mut key = None;
    for part in value.trim().split('+') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => chord.ctrl = true,
            "alt" | "option" => chord.alt = true,
            "shift" => chord.shift = true,
            "super" | "meta" | "cmd" => chord.super_key = true,
            other => key = Some(other.to_string()),
        }
    }
    chord.key = key?;
    Some(chord)
}

/// Parse a binding value that may contain alternatives separated by `|`.
pub fn parse(value: &str) -> Vec<Chord> {
    value
        .split('|')
        .filter_map(parse_chord)
        .collect()
}

/// Build the canonical string for a captured keystroke.
pub fn encode(key: &str, ctrl: bool, alt: bool, shift: bool, super_key: bool) -> String {
    let mut parts = Vec::new();
    if ctrl { parts.push("ctrl"); }
    if alt { parts.push("alt"); }
    if shift { parts.push("shift"); }
    if super_key { parts.push("super"); }
    parts.push(key);
    parts.join("+")
}

/// Apply a binding to a map, returning the conflicting action id on failure.
pub fn apply_binding(
    bindings: &mut BTreeMap<String, String>,
    action: &str,
    value: &str,
) -> Result<(), String> {
    if parse(value).is_empty() {
        return Err("invalid".into());
    }
    if let Some(conflict) = find_conflict(bindings, action, value) {
        return Err(conflict);
    }
    bindings.insert(action.to_string(), value.to_string());
    Ok(())
}

pub fn key_label(key: &str) -> String {
    match key.to_ascii_lowercase().as_str() {
        "comma" => ",".into(),
        "period" => ".".into(),
        "slash" => "/".into(),
        "space" => "Space".into(),
        "enter" => "Enter".into(),
        "escape" | "esc" => "Esc".into(),
        "tab" => "Tab".into(),
        "delete" => "Del".into(),
        "backspace" => "⌫".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "left" => "←".into(),
        "right" => "→".into(),
        other if other.chars().count() == 1 => other.to_uppercase(),
        other => other.to_string(),
    }
}

/// Find another action using one of the chords in `value`.
pub fn find_conflict(
    bindings: &BTreeMap<String, String>,
    action: &str,
    value: &str,
) -> Option<String> {
    let chords = parse(value);
    for (other, bound) in bindings {
        if other == action {
            continue;
        }
        for chord in parse(bound) {
            if chords.iter().any(|candidate| {
                candidate.key.eq_ignore_ascii_case(&chord.key)
                    && candidate.ctrl == chord.ctrl
                    && candidate.alt == chord.alt
                    && candidate.shift == chord.shift
                    && candidate.super_key == chord.super_key
            }) {
                return Some(other.clone());
            }
        }
    }
    None
}

pub fn label(value: &str) -> String {
    parse(value)
        .iter()
        .map(Chord::label)
        .collect::<Vec<_>>()
        .join(" / ")
}

pub fn matches(
    value: &str,
    key: &str,
    ctrl: bool,
    alt: bool,
    shift: bool,
    super_key: bool,
) -> bool {
    parse(value)
        .iter()
        .any(|chord| chord.matches(key, ctrl, alt, shift, super_key))
}

/// (action id, label) pairs shown in the settings editor.
pub const ACTIONS: &[(&str, &str)] = &[
    ("compose", "New message"),
    ("search", "Search"),
    ("settings", "Settings"),
    ("sync", "Sync now"),
    ("toggle_view", "Mail / Calendar"),
    ("next", "Next message"),
    ("previous", "Previous message"),
    ("open", "Open message"),
    ("archive", "Archive message"),
    ("trash", "Delete message"),
    ("mark_unread", "Mark read / unread"),
    ("copy", "Copy selected text"),
    ("select_all", "Select all text"),
    ("send", "Send message"),
    ("next_field", "Next field"),
    ("close", "Close overlay"),
    ("open_window", "Open message in new window"),
    ("snooze", "Snooze message"),
];

pub fn default_bindings() -> BTreeMap<String, String> {
    ACTIONS
        .iter()
        .map(|(id, _)| (id.to_string(), default_for(id).to_string()))
        .collect()
}

pub fn default_for(id: &str) -> &'static str {
    match id {
        "compose" => "ctrl+n",
        "search" => "ctrl+k",
        "settings" => "ctrl+,",
        "sync" => "ctrl+r",
        "toggle_view" => "ctrl+m|v",
        "next" => "j|down",
        "previous" => "k|up",
        "open" => "enter",
        "archive" => "e",
        "trash" => "#|delete",
        "mark_unread" => "u",
        "copy" => "ctrl+c",
        "select_all" => "ctrl+a",
        "send" => "ctrl+enter",
        "next_field" => "tab",
        "close" => "escape",
        "open_window" => "ctrl+shift+n",
        "snooze" => "s",
        _ => "",
    }
}

pub fn action_label(id: &str) -> &'static str {
    ACTIONS
        .iter()
        .find(|(action, _)| *action == id)
        .map(|(_, label)| *label)
        .unwrap_or("Unknown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modifiers_and_alternatives() {
        let chords = parse("ctrl+n|shift+#");
        assert_eq!(chords.len(), 2);
        assert!(chords[0].ctrl && chords[0].key == "n");
        assert!(chords[1].shift && chords[1].key == "#");
    }

    #[test]
    fn matches_chords() {
        assert!(matches("ctrl+n|shift+#", "n", true, false, false, false));
        assert!(matches("ctrl+n|shift+#", "#", false, false, true, false));
        assert!(!matches("ctrl+n|shift+#", "n", false, false, false, false));
    }

    #[test]
    fn displays_labels() {
        assert_eq!(label("ctrl+comma"), "Ctrl ,");
        assert_eq!(label("j|down"), "J / ↓");
    }

    #[test]
    fn defaults_cover_all_actions() {
        let defaults = default_bindings();
        assert_eq!(defaults.len(), ACTIONS.len());
        assert!(defaults.values().all(|value| !parse(value).is_empty()));
    }

    #[test]
    fn detects_conflicts() {
        let mut bindings = default_bindings();
        assert_eq!(find_conflict(&bindings, "sync", "ctrl+n"), Some("compose".into()));
        assert_eq!(find_conflict(&bindings, "sync", "ctrl+t"), None);
        bindings.remove("compose");
        assert_eq!(find_conflict(&bindings, "sync", "ctrl+n"), None);
    }

    #[test]
    fn applies_bindings() {
        let mut bindings = default_bindings();
        assert!(apply_binding(&mut bindings, "archive", "ctrl+t").is_ok());
        assert_eq!(bindings["archive"], "ctrl+t");
        assert_eq!(apply_binding(&mut bindings, "trash", "ctrl+t"), Err("archive".into()));
        assert!(apply_binding(&mut bindings, "archive", "not+a+valid+").is_ok());
    }
}
