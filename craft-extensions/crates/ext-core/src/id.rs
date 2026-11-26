//! Ids: who an extension is and which app it targets.
//!
//! Invariants: ids are lowercase ASCII from a tiny alphabet so look-alikes (case, Unicode homoglyphs)
//! cannot exist, and a validated id can be used as a folder name on every platform.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Why an id was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdError {
    Empty,
    TooLong,
    BadCharacter,
    BadShape,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            IdError::Empty => "the id is empty",
            IdError::TooLong => "the id is too long",
            IdError::BadCharacter => "the id may only use lowercase letters, digits, dots, dashes and underscores",
            IdError::BadShape => "the id must be dotted segments such as org.example.tone, each starting with a letter or digit",
        })
    }
}

impl std::error::Error for IdError {}

const MAX_EXTENSION_ID: usize = 64;
const MAX_APP_ID: usize = 32;

/// Prefixes only the first-party tier may use. A third-party package claiming one is refused (gate G1).
pub const FIRST_PARTY_PREFIXES: &[&str] =
    &["ai.storyteller.", "org.storytold.", "io.github.storytold.", "com.artcraft.", "ai.artcraft.", "com.getartcraft."];

/// A validated extension id such as `org.example.tone`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ExtensionId(String);

impl ExtensionId {
    pub fn new(text: &str) -> Result<Self, IdError> {
        if text.is_empty() {
            return Err(IdError::Empty);
        }
        if text.len() > MAX_EXTENSION_ID {
            return Err(IdError::TooLong);
        }
        if !text.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9' | '.' | '-' | '_')) {
            return Err(IdError::BadCharacter);
        }
        let mut segments = 0usize;
        for segment in text.split('.') {
            segments += 1;
            let edge_ok = |c: Option<char>| matches!(c, Some('a'..='z' | '0'..='9'));
            if !edge_ok(segment.chars().next()) || !edge_ok(segment.chars().next_back()) {
                return Err(IdError::BadShape);
            }
        }
        if segments < 2 {
            return Err(IdError::BadShape);
        }
        Ok(Self(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// True when the id sits under a prefix reserved for the first-party tier.
    pub fn in_first_party_namespace(&self) -> bool {
        FIRST_PARTY_PREFIXES.iter().any(|p| self.0.starts_with(p))
    }
}

impl fmt::Display for ExtensionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ExtensionId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        ExtensionId::new(&text).map_err(serde::de::Error::custom)
    }
}

/// A validated app id such as `photocraft`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct AppId(String);

impl AppId {
    pub fn new(text: &str) -> Result<Self, IdError> {
        if text.is_empty() {
            return Err(IdError::Empty);
        }
        if text.len() > MAX_APP_ID {
            return Err(IdError::TooLong);
        }
        if !text.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-')) {
            return Err(IdError::BadCharacter);
        }
        if !text.starts_with(|c: char| c.is_ascii_lowercase()) || text.ends_with('-') {
            return Err(IdError::BadShape);
        }
        Ok(Self(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for AppId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        AppId::new(&text).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_reverse_dns_ids() {
        for ok in ["org.example.tone", "io.github.someone.dark-ocean", "a1.b2", "x.y_z"] {
            assert!(ExtensionId::new(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn refuses_empty_and_too_long() {
        assert_eq!(ExtensionId::new(""), Err(IdError::Empty));
        let long = format!("a.{}", "b".repeat(63));
        assert_eq!(ExtensionId::new(&long), Err(IdError::TooLong));
        let exactly = format!("a.{}", "b".repeat(62));
        assert!(ExtensionId::new(&exactly).is_ok());
    }

    #[test]
    fn refuses_uppercase_unicode_and_punctuation() {
        for bad in ["Org.example", "org.exämple", "org.exаmple", "org.ex ample", "org.ex/ample", "org.ex\u{200b}ample", "org.ex\0"] {
            assert_eq!(ExtensionId::new(bad), Err(IdError::BadCharacter), "{bad:?}");
        }
    }

    #[test]
    fn refuses_bad_shapes() {
        for bad in ["single", ".leading", "trailing.", "double..dot", "org.-dash", "org._under", "org.a-", "org.a_"] {
            assert_eq!(ExtensionId::new(bad), Err(IdError::BadShape), "{bad:?}");
        }
    }

    #[test]
    fn first_party_namespaces_are_detected_exactly() {
        for p in FIRST_PARTY_PREFIXES {
            let id = ExtensionId::new(&format!("{p}tone")).unwrap();
            assert!(id.in_first_party_namespace(), "{p}");
        }
        assert!(!ExtensionId::new("org.example.tone").unwrap().in_first_party_namespace());
        // A look-alike that merely starts the same way is not under the prefix.
        assert!(!ExtensionId::new("org.storytoldx.tone").unwrap().in_first_party_namespace());
        assert!(!ExtensionId::new("org.storytold").is_ok() || !ExtensionId::new("org.storytold").unwrap().in_first_party_namespace());
    }

    #[test]
    fn app_ids_are_lowercase_words() {
        assert!(AppId::new("photocraft").is_ok());
        assert!(AppId::new("pdf-craft2").is_ok());
        for bad in ["", "PhotoCraft", "photo craft", "-photo", "photo.craft", "1photo"] {
            assert!(AppId::new(bad).is_err(), "{bad:?}");
        }
        assert_eq!(AppId::new(&"a".repeat(33)), Err(IdError::TooLong));
    }

    #[test]
    fn deserialize_validates() {
        assert!(serde_json::from_str::<ExtensionId>("\"org.example.tone\"").is_ok());
        assert!(serde_json::from_str::<ExtensionId>("\"Org.Example\"").is_err());
        assert!(serde_json::from_str::<AppId>("\"photocraft\"").is_ok());
        assert!(serde_json::from_str::<AppId>("\"Photo\"").is_err());
        assert_eq!(serde_json::to_string(&ExtensionId::new("org.example.tone").unwrap()).unwrap(), "\"org.example.tone\"");
    }
}
