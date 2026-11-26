//! UI localization (`en` English default, `ko` Korean / 한국어).
//!
//! Uses a static `OnceLock` translation table for Korean and a thread-local active locale so
//! parallel headless UI tests remain completely isolated across threads.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Supported UI display languages.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Locale {
    #[default]
    En,
    Ko,
}

impl Locale {
    pub const ALL: [Self; 2] = [Self::En, Self::Ko];

    /// Stable identifier used in commands (`ui.language`) and serialized UI state.
    pub const fn id(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Ko => "ko",
        }
    }

    /// Native display label shown in `Preferences › Display › Language`.
    pub const fn label(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Ko => "한국어 (Korean)",
        }
    }

    /// Parses a language code or display name.
    pub fn parse(input: &str) -> Option<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "en" | "en-us" | "english" => Some(Self::En),
            "ko" | "ko-kr" | "korean" | "한국어" | "한국어 (korean)" => Some(Self::Ko),
            _ => None,
        }
    }
}

std::thread_local! {
    static ACTIVE_LOCALE: Cell<Locale> = const { Cell::new(Locale::En) };
}

/// Sets the active UI locale on the current thread.
pub fn set_language(locale: Locale) {
    ACTIVE_LOCALE.with(|c| c.set(locale));
}

/// Returns the active UI locale on the current thread.
pub fn current_language() -> Locale {
    ACTIVE_LOCALE.with(Cell::get)
}

fn ko_table() -> &'static HashMap<String, &'static str> {
    static KO: OnceLock<HashMap<String, &'static str>> = OnceLock::new();
    KO.get_or_init(|| {
        let raw: HashMap<String, String> = serde_json::from_str(include_str!("../locales/ko.json")).unwrap_or_default();
        raw.into_iter()
            .map(|(k, v)| {
                let leaked: &'static str = Box::leak(v.into_boxed_str());
                (k, leaked)
            })
            .collect()
    })
}

/// Translates `key` into the active UI language, falling back to `key` itself when untranslated or English.
pub fn tr(key: &str) -> &str {
    match current_language() {
        Locale::En => key,
        Locale::Ko => ko_table().get(key).copied().unwrap_or(key),
    }
}

/// Number of entries loaded in the Korean (`ko`) translation catalog.
pub fn ko_entry_count() -> usize {
    ko_table().len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn korean_catalog_translates_core_menus_and_dsp_features_without_affecting_english_default() {
        assert!(ko_entry_count() >= 100);
        set_language(Locale::En);
        assert_eq!(tr("Separate Crosstalk"), "Separate Crosstalk");
        set_language(Locale::Ko);
        assert_eq!(tr("Separate Crosstalk"), "크로스토크 분리");
        assert_eq!(tr("Audio Leveling"), "오디오 레벨링");
        assert_eq!(tr("Stem Separation"), "보컬/반주/배경음 분리");
        assert_eq!(tr("Clip Looping"), "클립 루핑");
        assert_eq!(tr("MIDI Keyboard"), "MIDI 키보드");
        set_language(Locale::En);
    }

    #[test]
    fn ui_language_command_switches_between_english_and_korean() {
        let mut app = crate::SoundApp::new(soundcraft_engine::Engine::default(), None, crate::Services::default());
        assert_eq!(app.ui.language, Locale::En);
        assert_eq!(app.run("ui.language", json!({"language": "ko"})), Ok(json!({"language": "ko"})));
        assert_eq!(app.ui.language, Locale::Ko);
        assert_eq!(tr("File"), "파일");
        assert_eq!(app.run("ui.language", json!({"language": "en"})), Ok(json!({"language": "en"})));
        assert_eq!(app.ui.language, Locale::En);
        assert_eq!(tr("File"), "File");
    }
}
