//! Where the shared store lives on disk.
//!
//! Decision (Jeremy, 16/10/2026): the same shared folder approach as the AI store, under the person's
//! local data folder. The location never depends on the launcher's portable mode: a portable launcher
//! uses the same folder, because apps could not find a store hidden beside the launcher's exe. A person
//! who wants isolation sets `CRAFT_EXT_DIR` for the launcher and the apps alike.
//!
//! The environment is passed in as a lookup function, never read directly, so tests need no global state.

use std::fmt;
use std::path::PathBuf;

pub const OVERRIDE_VAR: &str = "CRAFT_EXT_DIR";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    Mac,
    Other,
}

impl Os {
    pub fn current() -> Os {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Other
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocationError {
    /// `CRAFT_EXT_DIR` is set but is not a full path.
    OverrideNotAbsolute,
    /// No home or data folder could be found.
    NoDataFolder,
}

impl fmt::Display for LocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LocationError::OverrideNotAbsolute => "the extensions folder setting must be a full folder path",
            LocationError::NoDataFolder => "this computer has no place to keep extensions",
        })
    }
}

impl std::error::Error for LocationError {}

/// Whether `text` is a full, tidy path in `os`'s style. Judged by the text, not the machine running the
/// test, so the rules behave the same everywhere.
fn looks_absolute(os: Os, text: &str) -> bool {
    let rooted = match os {
        Os::Windows => {
            let b = text.as_bytes();
            let drive = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/');
            drive || text.starts_with("\\\\")
        }
        _ => text.starts_with('/'),
    };
    rooted && !text.split(['/', '\\']).any(|segment| segment == "..")
}

/// The store folder for `os`, given an environment lookup.
pub fn root_for(os: Os, env: impl Fn(&str) -> Option<String>) -> Result<PathBuf, LocationError> {
    if let Some(custom) = env(OVERRIDE_VAR) {
        return if looks_absolute(os, &custom) { Ok(PathBuf::from(custom)) } else { Err(LocationError::OverrideNotAbsolute) };
    }
    let full = |name: &str| env(name).filter(|v| looks_absolute(os, v));
    let base = match os {
        Os::Windows => PathBuf::from(full("LOCALAPPDATA").ok_or(LocationError::NoDataFolder)?).join("ArtCraft").join("extensions"),
        Os::Mac => {
            let home = full("HOME").ok_or(LocationError::NoDataFolder)?;
            PathBuf::from(home).join("Library").join("Application Support").join("ArtCraft").join("extensions")
        }
        Os::Other => match full("XDG_DATA_HOME") {
            Some(data) => PathBuf::from(data).join("artcraft").join("extensions"),
            None => {
                let home = full("HOME").ok_or(LocationError::NoDataFolder)?;
                PathBuf::from(home).join(".local").join("share").join("artcraft").join("extensions")
            }
        },
    };
    Ok(base)
}

/// The store folder on this machine, from the real environment.
pub fn default_root() -> Result<PathBuf, LocationError> {
    root_for(Os::current(), |name| std::env::var(name).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn windows_uses_local_app_data() {
        let r = root_for(Os::Windows, env(&[("LOCALAPPDATA", "C:\\Users\\someone\\AppData\\Local")])).unwrap();
        assert_eq!(r, PathBuf::from("C:\\Users\\someone\\AppData\\Local").join("ArtCraft").join("extensions"));
    }

    #[test]
    fn mac_uses_application_support() {
        let r = root_for(Os::Mac, env(&[("HOME", "/Users/someone")])).unwrap();
        assert_eq!(r, PathBuf::from("/Users/someone/Library/Application Support/ArtCraft/extensions"));
    }

    #[test]
    fn other_systems_follow_xdg_then_home() {
        let r = root_for(Os::Other, env(&[("XDG_DATA_HOME", "/data"), ("HOME", "/home/someone")])).unwrap();
        assert_eq!(r, PathBuf::from("/data/artcraft/extensions"));
        let r = root_for(Os::Other, env(&[("HOME", "/home/someone")])).unwrap();
        assert_eq!(r, PathBuf::from("/home/someone/.local/share/artcraft/extensions"));
        let r = root_for(Os::Other, env(&[("XDG_DATA_HOME", "relative/dir"), ("HOME", "/home/someone")])).unwrap();
        assert_eq!(r, PathBuf::from("/home/someone/.local/share/artcraft/extensions"), "a relative XDG value is ignored");
    }

    #[test]
    fn the_override_wins_everywhere_and_must_be_a_full_clean_path() {
        for (os, abs) in [(Os::Windows, "C:\\isolated\\ext"), (Os::Mac, "/isolated/ext"), (Os::Other, "/isolated/ext")] {
            assert_eq!(root_for(os, env(&[(OVERRIDE_VAR, abs), ("HOME", "/h"), ("LOCALAPPDATA", "C:\\l")])).unwrap(), PathBuf::from(abs));
        }
        for bad in ["relative/ext", "", "..", "ext"] {
            assert_eq!(root_for(Os::Mac, env(&[(OVERRIDE_VAR, bad), ("HOME", "/h")])), Err(LocationError::OverrideNotAbsolute), "{bad:?}");
        }
        assert_eq!(root_for(Os::Mac, env(&[(OVERRIDE_VAR, "/a/../b")])), Err(LocationError::OverrideNotAbsolute));
        assert_eq!(root_for(Os::Windows, env(&[(OVERRIDE_VAR, "C:\\a\\..\\b")])), Err(LocationError::OverrideNotAbsolute));
        assert_eq!(root_for(Os::Windows, env(&[(OVERRIDE_VAR, "\\\\server\\share\\ext")])).unwrap(), PathBuf::from("\\\\server\\share\\ext"));
        assert_eq!(root_for(Os::Windows, env(&[(OVERRIDE_VAR, "/unix/style")])), Err(LocationError::OverrideNotAbsolute));
    }

    #[test]
    fn a_missing_data_folder_is_an_error_not_a_guess() {
        for os in [Os::Windows, Os::Mac, Os::Other] {
            assert_eq!(root_for(os, env(&[])), Err(LocationError::NoDataFolder), "{os:?}");
        }
        assert_eq!(root_for(Os::Windows, env(&[("LOCALAPPDATA", "")])), Err(LocationError::NoDataFolder));
        assert_eq!(root_for(Os::Windows, env(&[("LOCALAPPDATA", "relative")])), Err(LocationError::NoDataFolder));
    }
}
