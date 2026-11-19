use serde::{Deserialize, Serialize};

use crate::theme::omarchy::OmarchyColors;

/// The three appearance modes exposed in setup and Settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    White,
    Black,
    #[default]
    Omarchy,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::White => "White",
            Self::Black => "Black",
            Self::Omarchy => "Omarchy",
        }
    }

    pub fn is_dark(self) -> bool {
        !matches!(self, Self::White)
    }
}

/// A small, dependency-free RGBA color used by the theme layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn hex(value: u32) -> Self {
        Self {
            r: ((value >> 16) & 0xff) as u8,
            g: ((value >> 8) & 0xff) as u8,
            b: (value & 0xff) as u8,
            a: 255,
        }
    }

    /// Parse `#RGB`, `#RRGGBB`, or `#RRGGBBAA` (alpha last, CSS-style).
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim().trim_start_matches('#');
        match value.len() {
            3 => {
                let mut chars = value.chars();
                let r = chars.next()?.to_digit(16)? as u8;
                let g = chars.next()?.to_digit(16)? as u8;
                let b = chars.next()?.to_digit(16)? as u8;
                Some(Self::new(r * 17, g * 17, b * 17, 255))
            }
            6 => {
                let r = u8::from_str_radix(&value[0..2], 16).ok()?;
                let g = u8::from_str_radix(&value[2..4], 16).ok()?;
                let b = u8::from_str_radix(&value[4..6], 16).ok()?;
                Some(Self::new(r, g, b, 255))
            }
            8 => {
                let r = u8::from_str_radix(&value[0..2], 16).ok()?;
                let g = u8::from_str_radix(&value[2..4], 16).ok()?;
                let b = u8::from_str_radix(&value[4..6], 16).ok()?;
                let a = u8::from_str_radix(&value[6..8], 16).ok()?;
                Some(Self::new(r, g, b, a))
            }
            _ => None,
        }
    }

    pub fn with_alpha(self, alpha: f32) -> Self {
        Self {
            a: (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
            ..self
        }
    }

    /// Linear interpolation in sRGB. `t = 0` returns `self`, `t = 1` returns `other`.
    pub fn mix(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Self {
            r: lerp(self.r, other.r),
            g: lerp(self.g, other.g),
            b: lerp(self.b, other.b),
            a: lerp(self.a, other.a),
        }
    }

    /// Convert to the GPUI color type.
    pub fn gpui(self) -> gpui_kit::Rgba {
        gpui_kit::rgba(
            ((self.r as u32) << 24)
                | ((self.g as u32) << 16)
                | ((self.b as u32) << 8)
                | self.a as u32,
        )
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    pub fn luminance(self) -> f32 {
        fn channel(c: u8) -> f32 {
            let c = c as f32 / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    pub fn is_light(self) -> bool {
        self.luminance() > 0.5
    }

    pub fn contrast(self, other: Self) -> f32 {
        let a = self.luminance();
        let b = other.luminance();
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    pub fn ensure_contrast(self, background: Rgba, target: f32) -> Rgba {
        if self.contrast(background) >= target {
            return self;
        }
        let toward = if background.is_light() {
            Rgba::hex(0x000000)
        } else {
            Rgba::hex(0xFFFFFF)
        };
        let mut color = self;
        for _ in 0..24 {
            color = color.mix(toward, 0.08);
            if color.contrast(background) >= target {
                break;
            }
        }
        color
    }

    /// The more legible of near-black and white on top of this color.
    pub fn best_ink(self) -> Rgba {
        let white = Rgba::hex(0xFFFFFF);
        let black = Rgba::hex(0x111113);
        if white.contrast(self) >= black.contrast(self) {
            white
        } else {
            black
        }
    }
}

/// Semantic color tokens. Every surface in the app reads from this struct.
#[derive(Clone, Debug)]
pub struct Palette {
    pub mode: ThemeMode,
    pub is_dark: bool,
    pub canvas: Rgba,
    pub surface: Rgba,
    pub sidebar: Rgba,
    pub raised: Rgba,
    pub inset: Rgba,
    pub text: Rgba,
    pub muted: Rgba,
    pub faint: Rgba,
    pub border: Rgba,
    pub hover: Rgba,
    pub selection: Rgba,
    pub accent: Rgba,
    pub accent_ink: Rgba,
    pub danger: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
}

impl Palette {
    pub fn white(accent: Rgba) -> Self {
        let canvas = Rgba::hex(0xF5F5F7);
        let accent = accent.ensure_contrast(canvas, 3.0);
        Self {
            mode: ThemeMode::White,
            is_dark: false,
            canvas,
            surface: Rgba::hex(0xFFFFFF),
            sidebar: Rgba::hex(0xF0F0F3),
            raised: Rgba::hex(0xFFFFFF),
            inset: Rgba::hex(0xECECF0),
            text: Rgba::hex(0x1D1D1F),
            muted: Rgba::hex(0x6E6E73),
            faint: Rgba::hex(0x8E8E93),
            border: Rgba::hex(0xE3E3E8),
            hover: Rgba::hex(0x000000).with_alpha(0.035),
            selection: accent.with_alpha(0.12),
            accent,
            accent_ink: accent.best_ink(),
            danger: Rgba::hex(0xD70015),
            success: Rgba::hex(0x1A7F37),
            warning: Rgba::hex(0xB25000),
        }
    }

    pub fn black(accent: Rgba) -> Self {
        let canvas = Rgba::hex(0x0B0C0E);
        let accent = accent.ensure_contrast(canvas, 3.0);
        Self {
            mode: ThemeMode::Black,
            is_dark: true,
            canvas,
            surface: Rgba::hex(0x121316),
            sidebar: Rgba::hex(0x0E0F12),
            raised: Rgba::hex(0x191B1F),
            inset: Rgba::hex(0x08090B),
            text: Rgba::hex(0xF5F5F7),
            muted: Rgba::hex(0xA1A1A6),
            faint: Rgba::hex(0x6E6E73),
            border: Rgba::hex(0x23252A),
            hover: Rgba::hex(0xFFFFFF).with_alpha(0.05),
            selection: accent.with_alpha(0.22),
            accent,
            accent_ink: accent.best_ink(),
            danger: Rgba::hex(0xFF453A),
            success: Rgba::hex(0x30D158),
            warning: Rgba::hex(0xFF9F0A),
        }
    }

    pub fn from_omarchy(colors: &OmarchyColors, accent: Rgba) -> Self {
        let canvas = colors.background();
        let accent = accent.ensure_contrast(canvas, 3.0);
        let sidebar = colors.dark_background();
        let text = colors.foreground();
        Self {
            mode: ThemeMode::Omarchy,
            is_dark: colors.mode_is_dark(),
            canvas,
            surface: canvas,
            sidebar,
            raised: colors.lighter_background(),
            inset: colors.darker_background(),
            text,
            muted: colors.light_foreground(),
            faint: colors.dark_foreground(),
            border: colors.muted(),
            hover: sidebar.mix(text, 0.06),
            selection: colors.selection(),
            accent,
            accent_ink: accent.best_ink(),
            danger: colors.red(),
            success: colors.green(),
            warning: colors.yellow(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_variants() {
        assert_eq!(Rgba::parse("#fff").unwrap(), Rgba::hex(0xFFFFFF));
        assert_eq!(Rgba::parse("7fbbb3").unwrap(), Rgba::hex(0x7FBBB3));
        assert_eq!(Rgba::parse("#00000080").unwrap().a, 128);
    }

    #[test]
    fn ink_choice_is_legible() {
        let white = Rgba::hex(0xFFFFFF);
        let dark = Rgba::hex(0x111113);
        assert_eq!(white.best_ink(), dark);
        assert_eq!(dark.best_ink(), white);
    }

    #[test]
    fn accent_ensures_contrast() {
        let canvas = Rgba::hex(0x0B0C0E);
        let accent = Rgba::hex(0x7FBBB3).ensure_contrast(canvas, 4.5);
        assert!(accent.contrast(canvas) >= 4.5);
    }
}
