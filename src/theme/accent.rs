use crate::theme::palette::{Rgba, ThemeMode};

/// A selectable accent. `Blue` is the default and, in Omarchy mode, follows the
/// live desktop theme's blue token.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Accent {
    #[default]
    Blue,
    Violet,
    Pink,
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Graphite,
    Custom(Rgba),
}

impl Accent {
    pub const SWATCHES: [Accent; 9] = [
        Accent::Blue,
        Accent::Violet,
        Accent::Pink,
        Accent::Red,
        Accent::Orange,
        Accent::Yellow,
        Accent::Green,
        Accent::Teal,
        Accent::Graphite,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Blue => "Blue",
            Self::Violet => "Violet",
            Self::Pink => "Pink",
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Teal => "Teal",
            Self::Graphite => "Graphite",
            Self::Custom(_) => "Custom",
        }
    }

    pub fn resolve(self, mode: ThemeMode, omarchy_blue: Option<Rgba>) -> Rgba {
        let dark = mode.is_dark();
        let (light, dark_value) = match self {
            Self::Blue => (Rgba::hex(0x007AFF), Rgba::hex(0x0A84FF)),
            Self::Violet => (Rgba::hex(0x7C3AED), Rgba::hex(0xA78BFA)),
            Self::Pink => (Rgba::hex(0xDB2777), Rgba::hex(0xF472B6)),
            Self::Red => (Rgba::hex(0xDC2626), Rgba::hex(0xF87171)),
            Self::Orange => (Rgba::hex(0xEA580C), Rgba::hex(0xFB923C)),
            Self::Yellow => (Rgba::hex(0xCA8A04), Rgba::hex(0xFACC15)),
            Self::Green => (Rgba::hex(0x16A34A), Rgba::hex(0x4ADE80)),
            Self::Teal => (Rgba::hex(0x0D9488), Rgba::hex(0x2DD4BF)),
            Self::Graphite => (Rgba::hex(0x52525B), Rgba::hex(0xA1A1AA)),
            Self::Custom(color) => (color, color),
        };

        match mode {
            ThemeMode::White => light,
            ThemeMode::Black => dark_value,
            ThemeMode::Omarchy => {
                if matches!(self, Self::Blue) {
                    omarchy_blue.unwrap_or_else(|| Rgba::hex(0x7fbbb3))
                } else if dark {
                    dark_value
                } else {
                    light
                }
            }
        }
    }

    pub fn as_config_str(self) -> String {
        match self {
            Self::Blue => "blue".into(),
            Self::Violet => "violet".into(),
            Self::Pink => "pink".into(),
            Self::Red => "red".into(),
            Self::Orange => "orange".into(),
            Self::Yellow => "yellow".into(),
            Self::Green => "green".into(),
            Self::Teal => "teal".into(),
            Self::Graphite => "graphite".into(),
            Self::Custom(color) => color.to_hex(),
        }
    }

    pub fn from_config_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "blue" => Self::Blue,
            "violet" => Self::Violet,
            "pink" => Self::Pink,
            "red" => Self::Red,
            "orange" => Self::Orange,
            "yellow" => Self::Yellow,
            "green" => Self::Green,
            "teal" => Self::Teal,
            "graphite" => Self::Graphite,
            other => Rgba::parse(other)
                .map(Self::Custom)
                .unwrap_or(Self::Blue),
        }
    }
}
