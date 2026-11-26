//! The shared theme: a small palette every app can map onto its own tokens, plus optional per-app
//! overrides in that app's own token names.
//!
//! Invariants: a theme is data only. The palette must keep text readable (WCAG contrast, the same
//! 4.5:1 rule PhotoCraft already tests) so a theme cannot be installed that hides the interface.
//! Overrides use names only the app understands, so the app re-checks its own pairs after applying them.

use crate::id::AppId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub const MAX_THEME_BYTES: usize = 64 * 1024;
pub const SCHEMA: u32 = 1;
pub const MAX_OVERRIDE_APPS: usize = 32;
pub const MAX_TOKENS_PER_APP: usize = 64;

/// An opaque colour written as `#rrggbb`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn parse(text: &str) -> Option<Rgb> {
        let digits = text.strip_prefix('#')?;
        if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| digits.get(i..i + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok());
        Some(Rgb(byte(0)?, byte(2)?, byte(4)?))
    }

    /// Relative luminance as WCAG defines it, 0.0 (black) to 1.0 (white).
    pub fn luminance(self) -> f64 {
        let linear = |v: u8| {
            let c = f64::from(v) / 255.0;
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * linear(self.0) + 0.7152 * linear(self.1) + 0.0722 * linear(self.2)
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

impl Serialize for Rgb {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Rgb::parse(&text).ok_or_else(|| serde::de::Error::custom("a colour looks like #1a2b3c"))
    }
}

/// WCAG contrast ratio between two colours, 1.0 to 21.0.
pub fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette {
    /// True for a dark theme (light text on a dark background).
    pub dark: bool,
    pub background: Rgb,
    pub surface: Rgb,
    pub text: Rgb,
    pub muted_text: Rgb,
    pub line: Rgb,
    pub accent: Rgb,
    pub good: Rgb,
    pub warn: Rgb,
    pub bad: Rgb,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeFile {
    pub schema: u32,
    pub palette: Palette,
    /// Per-app overrides keyed by app id, then by that app's own token name.
    #[serde(default)]
    pub apps: BTreeMap<AppId, BTreeMap<String, Rgb>>,
}

/// Which pair of colours was too close together to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pair {
    TextOnBackground,
    TextOnSurface,
    MutedTextOnBackground,
    MutedTextOnSurface,
    AccentOnBackground,
    GoodOnBackground,
    WarnOnBackground,
    BadOnBackground,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeError {
    TooLarge,
    NotReadable,
    UnsupportedSchema(u32),
    LowContrast(Pair),
    InconsistentMode,
    TooManyApps,
    TooManyTokens,
    BadTokenName,
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeError::TooLarge => f.write_str("the theme file is too large"),
            ThemeError::NotReadable => f.write_str("the theme file cannot be read"),
            ThemeError::UnsupportedSchema(n) => write!(f, "the theme uses a format version ({n}) this app does not know"),
            ThemeError::LowContrast(pair) => write!(f, "the theme's {} are too close in colour to read", pair.describe()),
            ThemeError::InconsistentMode => f.write_str("the theme says it is dark or light but its colours say otherwise"),
            ThemeError::TooManyApps => f.write_str("the theme tunes too many apps"),
            ThemeError::TooManyTokens => f.write_str("the theme tunes too many colours for one app"),
            ThemeError::BadTokenName => f.write_str("the theme names a colour in a way that is not allowed"),
        }
    }
}

impl Pair {
    fn describe(self) -> &'static str {
        match self {
            Pair::TextOnBackground => "text and background",
            Pair::TextOnSurface => "text and panel colours",
            Pair::MutedTextOnBackground => "secondary text and background",
            Pair::MutedTextOnSurface => "secondary text and panel colours",
            Pair::AccentOnBackground => "highlight and background",
            Pair::GoodOnBackground => "success colour and background",
            Pair::WarnOnBackground => "warning colour and background",
            Pair::BadOnBackground => "error colour and background",
        }
    }
}

impl std::error::Error for ThemeError {}

#[derive(Deserialize)]
struct Head {
    schema: u32,
}

/// Minimum contrast: 4.5:1 for text, 3:1 for colours that are shapes or status marks, as WCAG asks.
const TEXT_RATIO: f64 = 4.5;
const MARK_RATIO: f64 = 3.0;

fn valid_token_name(name: &str) -> bool {
    name.len() <= 32 && name.starts_with(|c: char| c.is_ascii_lowercase()) && name.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_'))
}

impl Palette {
    fn check(&self) -> Result<(), ThemeError> {
        if self.dark != (self.background.luminance() < self.text.luminance()) {
            return Err(ThemeError::InconsistentMode);
        }
        let rows = [
            (self.text, self.background, TEXT_RATIO, Pair::TextOnBackground),
            (self.text, self.surface, TEXT_RATIO, Pair::TextOnSurface),
            (self.muted_text, self.background, TEXT_RATIO, Pair::MutedTextOnBackground),
            (self.muted_text, self.surface, TEXT_RATIO, Pair::MutedTextOnSurface),
            (self.accent, self.background, MARK_RATIO, Pair::AccentOnBackground),
            (self.good, self.background, MARK_RATIO, Pair::GoodOnBackground),
            (self.warn, self.background, MARK_RATIO, Pair::WarnOnBackground),
            (self.bad, self.background, MARK_RATIO, Pair::BadOnBackground),
        ];
        for (front, back, need, pair) in rows {
            if contrast_ratio(front, back) < need {
                return Err(ThemeError::LowContrast(pair));
            }
        }
        Ok(())
    }
}

impl ThemeFile {
    pub fn parse(bytes: &[u8]) -> Result<Self, ThemeError> {
        if bytes.len() > MAX_THEME_BYTES {
            return Err(ThemeError::TooLarge);
        }
        let head: Head = serde_json::from_slice(bytes).map_err(|_| ThemeError::NotReadable)?;
        if head.schema != SCHEMA {
            return Err(ThemeError::UnsupportedSchema(head.schema));
        }
        let theme: ThemeFile = serde_json::from_slice(bytes).map_err(|_| ThemeError::NotReadable)?;
        if theme.apps.len() > MAX_OVERRIDE_APPS {
            return Err(ThemeError::TooManyApps);
        }
        for tokens in theme.apps.values() {
            if tokens.len() > MAX_TOKENS_PER_APP {
                return Err(ThemeError::TooManyTokens);
            }
            if !tokens.keys().all(|name| valid_token_name(name)) {
                return Err(ThemeError::BadTokenName);
            }
        }
        theme.palette.check()?;
        Ok(theme)
    }

    pub fn overrides_for(&self, app: &AppId) -> Option<&BTreeMap<String, Rgb>> {
        self.apps.get(app)
    }
}

#[cfg(test)]
mod tests;
