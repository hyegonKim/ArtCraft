//! The state file: what is installed, enabled for which app, and which theme each app uses.
//!
//! Invariants: read with a size cap; a newer schema is refused rather than half understood; a missing
//! file is an empty state; every change made through the store bumps `generation`, which running apps
//! watch to notice an install without any other signal.

use craft_ext_core::id::{AppId, ExtensionId};
use craft_ext_core::manifest::Kind;
use craft_ext_core::tier::Tier;
use craft_ext_core::version::Version;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const STATE_FILE: &str = "state.json";
pub const MAX_STATE_BYTES: u64 = 4 * 1024 * 1024;
pub const SCHEMA: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installed {
    pub id: ExtensionId,
    pub version: Version,
    pub kind: Kind,
    pub name: String,
    /// Where it came from. Assigned by the host at install, never read from the package.
    pub tier: Tier,
    /// Apps the package says it works with.
    pub apps: Vec<AppId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub schema: u32,
    pub generation: u64,
    #[serde(default)]
    pub installed: Vec<Installed>,
    #[serde(default)]
    pub enabled: BTreeMap<AppId, BTreeSet<ExtensionId>>,
    #[serde(default)]
    pub active_theme: BTreeMap<AppId, ExtensionId>,
}

impl Default for State {
    fn default() -> Self {
        Self { schema: SCHEMA, generation: 0, installed: Vec::new(), enabled: BTreeMap::new(), active_theme: BTreeMap::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateError {
    TooLarge,
    Corrupt,
    Newer(u32),
    Unreadable,
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            StateError::TooLarge => "the list of installed extensions is too large to read",
            StateError::Corrupt => "the list of installed extensions is damaged",
            StateError::Newer(_) => "the list of installed extensions was saved by a newer version of ArtCraft",
            StateError::Unreadable => "the list of installed extensions cannot be opened",
        })
    }
}

impl std::error::Error for StateError {}

#[derive(Deserialize)]
struct Head {
    schema: u32,
}

impl State {
    /// Reads the state file. A missing file is an empty state; anything else wrong is an error, and the
    /// file is left exactly as it was.
    pub fn read(path: &Path) -> Result<State, StateError> {
        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(State::default()),
            Err(_) => return Err(StateError::Unreadable),
        };
        let mut bytes = Vec::new();
        file.take(MAX_STATE_BYTES + 1).read_to_end(&mut bytes).map_err(|_| StateError::Unreadable)?;
        if bytes.len() as u64 > MAX_STATE_BYTES {
            return Err(StateError::TooLarge);
        }
        let head: Head = serde_json::from_slice(&bytes).map_err(|_| StateError::Corrupt)?;
        if head.schema > SCHEMA {
            return Err(StateError::Newer(head.schema));
        }
        let state: State = serde_json::from_slice(&bytes).map_err(|_| StateError::Corrupt)?;
        if state.schema != SCHEMA {
            return Err(StateError::Corrupt);
        }
        Ok(state)
    }

    pub fn find(&self, id: &ExtensionId) -> Option<&Installed> {
        self.installed.iter().find(|i| &i.id == id)
    }

    pub fn is_enabled(&self, app: &AppId, id: &ExtensionId) -> bool {
        self.enabled.get(app).is_some_and(|set| set.contains(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, text: &str) -> std::path::PathBuf {
        let p = dir.join(STATE_FILE);
        std::fs::write(&p, text).unwrap();
        p
    }

    #[test]
    fn a_missing_file_is_an_empty_state() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(State::read(&dir.path().join(STATE_FILE)).unwrap(), State::default());
    }

    #[test]
    fn a_saved_state_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = State { generation: 4, ..State::default() };
        let app = AppId::new("photocraft").unwrap();
        let id = ExtensionId::new("org.example.tone").unwrap();
        s.installed.push(Installed {
            id: id.clone(),
            version: Version::new(1, 2, 3),
            kind: Kind::Theme,
            name: "Tone".into(),
            tier: Tier::FromFile,
            apps: vec![app.clone()],
        });
        s.enabled.entry(app.clone()).or_default().insert(id.clone());
        s.active_theme.insert(app.clone(), id.clone());
        let p = write(dir.path(), &serde_json::to_string(&s).unwrap());
        let back = State::read(&p).unwrap();
        assert_eq!(back, s);
        assert!(back.is_enabled(&app, &id) && back.find(&id).is_some());
        assert!(!back.is_enabled(&AppId::new("filmcraft").unwrap(), &id));
    }

    #[test]
    fn newer_corrupt_and_oversize_files_are_refused_and_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(dir.path(), r#"{"schema":2,"generation":1,"future":true}"#);
        assert_eq!(State::read(&p), Err(StateError::Newer(2)));
        assert_eq!(std::fs::read_to_string(&p).unwrap(), r#"{"schema":2,"generation":1,"future":true}"#);
        for bad in ["", "{", "[]", r#"{"schema":1}"#, r#"{"schema":0,"generation":0}"#, r#"{"schema":1,"generation":"x"}"#] {
            let p = write(dir.path(), bad);
            assert_eq!(State::read(&p), Err(StateError::Corrupt), "{bad:?}");
        }
        let p = write(dir.path(), &" ".repeat(MAX_STATE_BYTES as usize + 1));
        assert_eq!(State::read(&p), Err(StateError::TooLarge));
        // A folder where the file should be is unreadable, not a panic.
        let sub = tempfile::tempdir().unwrap();
        std::fs::create_dir(sub.path().join(STATE_FILE)).unwrap();
        assert_eq!(State::read(&sub.path().join(STATE_FILE)), Err(StateError::Unreadable));
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(dir.path(), r#"{"schema":1,"generation":9,"extra":{"a":1}}"#);
        assert_eq!(State::read(&p).unwrap().generation, 9);
    }
}
