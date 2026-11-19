use serde::Deserialize;
use std::path::PathBuf;

use crate::theme::palette::{Palette, Rgba};

/// The subset of Omarchy's `colors.toml` that maps to app surfaces.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct OmarchyColors {
    pub mode: String,
    pub accent: String,
    pub selection: String,
    pub muted: String,
    pub background: String,
    pub dark_background: String,
    pub darker_background: String,
    pub lighter_background: String,
    pub foreground: String,
    pub light_foreground: String,
    pub dark_foreground: String,
    pub red: String,
    pub yellow: String,
    pub green: String,
    pub blue: String,
    pub magenta: String,
    pub orange: String,
}

impl OmarchyColors {
    pub fn mode_is_dark(&self) -> bool {
        !self.mode.eq_ignore_ascii_case("light")
    }

    fn color(&self, value: &str, fallback: u32) -> Rgba {
        Rgba::parse(value).unwrap_or_else(|| Rgba::hex(fallback))
    }

    pub fn background(&self) -> Rgba {
        self.color(&self.background, 0x2d353b)
    }

    pub fn dark_background(&self) -> Rgba {
        self.color(&self.dark_background, 0x21272c)
    }

    pub fn darker_background(&self) -> Rgba {
        self.color(&self.darker_background, 0x181d20)
    }

    pub fn lighter_background(&self) -> Rgba {
        self.color(&self.lighter_background, 0x343f44)
    }

    pub fn foreground(&self) -> Rgba {
        self.color(&self.foreground, 0xd3c6aa)
    }

    pub fn light_foreground(&self) -> Rgba {
        self.color(&self.light_foreground, 0x9da9a0)
    }

    pub fn dark_foreground(&self) -> Rgba {
        self.color(&self.dark_foreground, 0x4f585e)
    }

    pub fn selection(&self) -> Rgba {
        self.color(&self.selection, 0x3d484d)
    }

    pub fn muted(&self) -> Rgba {
        self.color(&self.muted, 0x475258)
    }

    pub fn blue(&self) -> Rgba {
        self.color(&self.blue, 0x7fbbb3)
    }

    pub fn red(&self) -> Rgba {
        self.color(&self.red, 0xe67e80)
    }

    pub fn yellow(&self) -> Rgba {
        self.color(&self.yellow, 0xdbbc7f)
    }

    pub fn green(&self) -> Rgba {
        self.color(&self.green, 0xa7c080)
    }

    pub fn palette(&self, accent: Rgba) -> Palette {
        Palette::from_omarchy(self, accent)
    }
}

/// Where Omarchy publishes the currently active theme.
pub fn live_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let candidates = [
        PathBuf::from(&home).join(".local/state/omarchy/current/theme/colors.toml"),
        PathBuf::from(&home).join(".config/omarchy/current/theme/colors.toml"),
    ];
    candidates.into_iter().find(|path| path.exists())
}

pub fn read_live() -> Option<OmarchyColors> {
    let path = live_path()?;
    read_from(&path)
}

pub fn read_from(path: &std::path::Path) -> Option<OmarchyColors> {
    let raw = std::fs::read_to_string(path).ok()?;
    toml::from_str(&raw).ok()
}

pub fn current_theme_name() -> Option<String> {
    let home = std::env::var_os("HOME")?;
    let link = PathBuf::from(home).join(".local/state/omarchy/current");
    let target = std::fs::read_link(&link).ok()?;
    target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}
