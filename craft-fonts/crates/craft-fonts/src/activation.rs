//! Font Activation Set (`FontActivationRegistry`): activate/deactivate font families by ID with
//! deterministic, zero-dependency JSON state serialization.

use std::collections::BTreeSet;
use std::fmt;

use crate::packs::{FontPackCategory, font_pack};
use crate::{FONTS, Font, LATIN_FONTS};

const STATE_SCHEMA_VERSION: u32 = 1;

/// Canonicalize a font family name or slug into a lowercase hyphenated family ID
/// (e.g. `"Source Serif 4"` -> `"source-serif-4"`, `"Noto Sans CJK SC"` -> `"noto-sans-cjk-sc"`).
pub fn family_id(family_or_id: &str) -> String {
    let mut out = String::with_capacity(family_or_id.len());
    let mut prev_dash = true;
    for ch in family_or_id.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if (ch.is_whitespace() || matches!(ch, '-' | '_' | '.')) && !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Error returned when deserializing a [`FontActivationRegistry`] state payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActivationStateError {
    /// JSON syntax or structural mismatch.
    MalformedJson,
    /// State `version` field is missing or unsupported.
    UnsupportedVersion(u32),
    /// An entry in `active` normalized to an empty family ID.
    InvalidFamilyId,
}

impl fmt::Display for ActivationStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedJson => f.write_str("activation state JSON is malformed"),
            Self::UnsupportedVersion(v) => {
                write!(f, "unsupported activation state schema version {v}")
            }
            Self::InvalidFamilyId => f.write_str("activation state contains an empty family id"),
        }
    }
}

impl std::error::Error for ActivationStateError {}

/// Tracks activated Creative Cloud / Craft Suite font families by canonical ID.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FontActivationRegistry {
    active: BTreeSet<String>,
}

impl FontActivationRegistry {
    /// Create an empty activation registry with no families active.
    pub fn new() -> Self {
        Self {
            active: BTreeSet::new(),
        }
    }

    /// Create a registry pre-populated with every default family from [`FONTS`].
    pub fn with_default_manifest_active() -> Self {
        let mut reg = Self::new();
        for f in FONTS {
            reg.activate(f.family);
        }
        reg
    }

    /// Activate a font family by name or canonical ID.
    /// Returns `true` if the family became newly active, or `false` if already active or empty.
    pub fn activate(&mut self, family_or_id: &str) -> bool {
        let id = family_id(family_or_id);
        if id.is_empty() {
            return false;
        }
        self.active.insert(id)
    }

    /// Deactivate a font family by name or canonical ID.
    /// Returns `true` if the family was active and has now been deactivated.
    pub fn deactivate(&mut self, family_or_id: &str) -> bool {
        let id = family_id(family_or_id);
        self.active.remove(&id)
    }

    /// Activate every curated family in `category`, returning the count of newly activated families.
    pub fn activate_pack(&mut self, category: FontPackCategory) -> usize {
        let mut added = 0;
        for specimen in font_pack(category).specimens {
            if self.activate(specimen.family) {
                added += 1;
            }
        }
        added
    }

    /// Deactivate every curated family in `category`, returning the count of removed families.
    pub fn deactivate_pack(&mut self, category: FontPackCategory) -> usize {
        let mut removed = 0;
        for specimen in font_pack(category).specimens {
            if self.deactivate(specimen.family) {
                removed += 1;
            }
        }
        removed
    }

    /// Check whether `family_or_id` is currently active.
    pub fn is_active(&self, family_or_id: &str) -> bool {
        let id = family_id(family_or_id);
        !id.is_empty() && self.active.contains(&id)
    }

    /// Number of active font families.
    pub fn len(&self) -> usize {
        self.active.len()
    }

    /// Whether zero font families are currently active.
    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }

    /// Deterministic sorted list of active canonical family IDs.
    pub fn active_ids(&self) -> Vec<&str> {
        self.active.iter().map(String::as_str).collect()
    }

    /// Resolve all [`Font`] faces in [`FONTS`] and [`LATIN_FONTS`] whose family is currently active.
    pub fn active_catalog_fonts(&self) -> Vec<&'static Font> {
        FONTS
            .iter()
            .chain(LATIN_FONTS.iter())
            .filter(|f| self.is_active(f.family))
            .collect()
    }

    /// Serialize the activation set to deterministic compact JSON (`{"version":1,"active":[...]}`).
    pub fn to_json(&self) -> String {
        let mut out = format!("{{\"version\":{STATE_SCHEMA_VERSION},\"active\":[");
        for (idx, id) in self.active.iter().enumerate() {
            if idx > 0 {
                out.push(',');
            }
            out.push('"');
            for ch in id.chars() {
                if ch == '"' || ch == '\\' {
                    out.push('\\');
                }
                out.push(ch);
            }
            out.push('"');
        }
        out.push_str("]}");
        out
    }

    /// Deserialize an activation set previously produced by [`Self::to_json`].
    pub fn from_json(json: &str) -> Result<Self, ActivationStateError> {
        let bytes = json.as_bytes();
        let mut pos = skip_ws(bytes, 0);
        if bytes.get(pos) != Some(&b'{') {
            return Err(ActivationStateError::MalformedJson);
        }
        pos = skip_ws(bytes, pos + 1);

        let mut seen_version: Option<u32> = None;
        let mut seen_active: Option<BTreeSet<String>> = None;

        if bytes.get(pos) == Some(&b'}') {
            return Err(ActivationStateError::MalformedJson);
        }

        loop {
            let (key, next_pos) = parse_json_string(bytes, pos)?;
            pos = skip_ws(bytes, next_pos);
            if bytes.get(pos) != Some(&b':') {
                return Err(ActivationStateError::MalformedJson);
            }
            pos = skip_ws(bytes, pos + 1);

            match key.as_str() {
                "version" => {
                    let (ver, after_ver) = parse_json_u32(bytes, pos)?;
                    seen_version = Some(ver);
                    pos = skip_ws(bytes, after_ver);
                }
                "active" => {
                    let (items, after_arr) = parse_json_string_array(bytes, pos)?;
                    let mut set = BTreeSet::new();
                    for raw in items {
                        let normalized = family_id(&raw);
                        if normalized.is_empty() {
                            return Err(ActivationStateError::InvalidFamilyId);
                        }
                        set.insert(normalized);
                    }
                    seen_active = Some(set);
                    pos = skip_ws(bytes, after_arr);
                }
                _ => return Err(ActivationStateError::MalformedJson),
            }

            match bytes.get(pos) {
                Some(&b',') => {
                    pos = skip_ws(bytes, pos + 1);
                }
                Some(&b'}') => {
                    pos = skip_ws(bytes, pos + 1);
                    break;
                }
                _ => return Err(ActivationStateError::MalformedJson),
            }
        }

        if pos != bytes.len() {
            return Err(ActivationStateError::MalformedJson);
        }

        let version = seen_version.ok_or(ActivationStateError::MalformedJson)?;
        if version != STATE_SCHEMA_VERSION {
            return Err(ActivationStateError::UnsupportedVersion(version));
        }
        let active = seen_active.ok_or(ActivationStateError::MalformedJson)?;
        Ok(Self { active })
    }
}

fn skip_ws(bytes: &[u8], mut pos: usize) -> usize {
    while let Some(&b) = bytes.get(pos) {
        if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
            pos += 1;
        } else {
            break;
        }
    }
    pos
}

fn parse_json_u32(bytes: &[u8], start: usize) -> Result<(u32, usize), ActivationStateError> {
    let mut pos = start;
    while let Some(&b) = bytes.get(pos) {
        if b.is_ascii_digit() {
            pos += 1;
        } else {
            break;
        }
    }
    if pos == start {
        return Err(ActivationStateError::MalformedJson);
    }
    let slice = bytes
        .get(start..pos)
        .ok_or(ActivationStateError::MalformedJson)?;
    let text = std::str::from_utf8(slice).map_err(|_| ActivationStateError::MalformedJson)?;
    let val = text
        .parse::<u32>()
        .map_err(|_| ActivationStateError::MalformedJson)?;
    Ok((val, pos))
}

fn parse_json_string(bytes: &[u8], start: usize) -> Result<(String, usize), ActivationStateError> {
    if bytes.get(start) != Some(&b'"') {
        return Err(ActivationStateError::MalformedJson);
    }
    let mut pos = start + 1;
    let mut out = Vec::new();
    while let Some(&b) = bytes.get(pos) {
        pos += 1;
        match b {
            b'"' => {
                let s = String::from_utf8(out).map_err(|_| ActivationStateError::MalformedJson)?;
                return Ok((s, pos));
            }
            b'\\' => {
                let esc = *bytes.get(pos).ok_or(ActivationStateError::MalformedJson)?;
                pos += 1;
                match esc {
                    b'"' | b'\\' | b'/' => out.push(esc),
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    _ => return Err(ActivationStateError::MalformedJson),
                }
            }
            0x00..=0x1F => return Err(ActivationStateError::MalformedJson),
            other => out.push(other),
        }
    }
    Err(ActivationStateError::MalformedJson)
}

fn parse_json_string_array(
    bytes: &[u8],
    start: usize,
) -> Result<(Vec<String>, usize), ActivationStateError> {
    if bytes.get(start) != Some(&b'[') {
        return Err(ActivationStateError::MalformedJson);
    }
    let mut pos = skip_ws(bytes, start + 1);
    let mut items = Vec::new();
    if bytes.get(pos) == Some(&b']') {
        return Ok((items, pos + 1));
    }
    loop {
        let (item, next_pos) = parse_json_string(bytes, pos)?;
        items.push(item);
        pos = skip_ws(bytes, next_pos);
        match bytes.get(pos) {
            Some(&b',') => {
                pos = skip_ws(bytes, pos + 1);
            }
            Some(&b']') => return Ok((items, pos + 1)),
            _ => return Err(ActivationStateError::MalformedJson),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_deactivate_and_round_trip_json() {
        let mut reg = FontActivationRegistry::new();
        assert!(reg.is_empty());
        assert!(reg.activate("Source Serif 4"));
        assert!(!reg.activate("source-serif-4"));
        assert!(reg.activate("Inter"));
        assert!(reg.is_active("SOURCE SERIF 4"));
        assert_eq!(reg.active_ids(), vec!["inter", "source-serif-4"]);
        assert_eq!(reg.active_catalog_fonts().len(), 8); // 4 Inter + 4 Source Serif 4

        let json = reg.to_json();
        assert_eq!(json, r#"{"version":1,"active":["inter","source-serif-4"]}"#);

        let restored = FontActivationRegistry::from_json(&json).unwrap();
        assert_eq!(restored, reg);

        assert!(reg.deactivate("Inter"));
        assert!(!reg.is_active("inter"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn pack_activation_and_error_cases() {
        let mut reg = FontActivationRegistry::new();
        let added = reg.activate_pack(FontPackCategory::GlobalCjkTypography);
        assert_eq!(added, 5);
        assert!(reg.is_active("NanumGothic"));
        assert!(reg.is_active("Shippori Mincho"));
        assert_eq!(
            reg.deactivate_pack(FontPackCategory::GlobalCjkTypography),
            5
        );
        assert!(reg.is_empty());

        assert_eq!(
            FontActivationRegistry::from_json(r#"{"version":2,"active":[]}"#),
            Err(ActivationStateError::UnsupportedVersion(2))
        );
        assert_eq!(
            FontActivationRegistry::from_json(r#"{"version":1,"active":["   "]}"#),
            Err(ActivationStateError::InvalidFamilyId)
        );
        assert_eq!(
            FontActivationRegistry::from_json("{broken"),
            Err(ActivationStateError::MalformedJson)
        );
    }
}
