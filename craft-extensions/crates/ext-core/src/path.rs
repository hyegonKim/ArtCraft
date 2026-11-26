//! Paths of files inside a package. Always relative, forward slashes, plain names.
//!
//! Invariant: a validated path cannot leave its folder and is a legal file name on Windows, macOS and
//! Linux, so unpacking never has to guess.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathError {
    Empty,
    TooLong,
    TooDeep,
    EmptySegment,
    DotSegment,
    BadCharacter,
    TrailingDot,
    ReservedName,
    CaseCollision,
    Duplicate,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PathError::Empty => "a file path is empty",
            PathError::TooLong => "a file path is too long",
            PathError::TooDeep => "a file path has too many folders",
            PathError::EmptySegment => "a file path has an empty folder name",
            PathError::DotSegment => "a file path steps out of its folder",
            PathError::BadCharacter => "a file path uses a character that is not allowed",
            PathError::TrailingDot => "a file name ends with a dot",
            PathError::ReservedName => "a file name is reserved by the system",
            PathError::CaseCollision => "two files differ only in letter case",
            PathError::Duplicate => "a file is listed twice",
        })
    }
}

impl std::error::Error for PathError {}

pub const MAX_PATH_BYTES: usize = 200;
pub const MAX_SEGMENT_BYTES: usize = 64;
pub const MAX_DEPTH: usize = 8;

const RESERVED: &[&str] = &["con", "prn", "aux", "nul"];

fn is_reserved_device(segment: &str) -> bool {
    // Windows reserves the name before the first dot: "nul.txt" is as dead as "nul".
    let stem = segment.split('.').next().unwrap_or("").to_ascii_lowercase();
    if RESERVED.contains(&stem.as_str()) {
        return true;
    }
    let digit = |s: &str, prefix: &str| s.strip_prefix(prefix).is_some_and(|d| matches!(d.as_bytes(), [b'1'..=b'9']));
    digit(&stem, "com") || digit(&stem, "lpt")
}

pub fn validate_path(path: &str) -> Result<(), PathError> {
    if path.is_empty() {
        return Err(PathError::Empty);
    }
    if path.len() > MAX_PATH_BYTES {
        return Err(PathError::TooLong);
    }
    let mut depth = 0usize;
    for segment in path.split('/') {
        depth += 1;
        if segment.is_empty() {
            return Err(PathError::EmptySegment);
        }
        if segment == "." || segment == ".." {
            return Err(PathError::DotSegment);
        }
        if !segment.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')) {
            return Err(PathError::BadCharacter);
        }
        if segment.ends_with('.') {
            return Err(PathError::TrailingDot);
        }
        if segment.len() > MAX_SEGMENT_BYTES {
            return Err(PathError::TooLong);
        }
        if is_reserved_device(segment) {
            return Err(PathError::ReservedName);
        }
    }
    if depth > MAX_DEPTH {
        return Err(PathError::TooDeep);
    }
    Ok(())
}

/// Checks a whole file list: every path valid, none repeated, none differing only by case (or being
/// a file where another has a folder).
pub fn validate_all<'a>(paths: impl IntoIterator<Item = &'a str>) -> Result<(), PathError> {
    let mut exact: BTreeSet<&str> = BTreeSet::new();
    let mut files: BTreeSet<String> = BTreeSet::new();
    // Folder prefix (lower case) -> the spelling first seen. A second spelling is a collision.
    let mut folders: BTreeMap<String, &str> = BTreeMap::new();
    for path in paths {
        validate_path(path)?;
        if !exact.insert(path) {
            return Err(PathError::Duplicate);
        }
        let mut end = 0usize;
        let segments: Vec<&str> = path.split('/').collect();
        for (i, segment) in segments.iter().enumerate() {
            end += segment.len() + usize::from(i > 0);
            let prefix = path.get(..end).unwrap_or(path);
            let key = prefix.to_ascii_lowercase();
            if i + 1 < segments.len() {
                if files.contains(&key) {
                    return Err(PathError::CaseCollision);
                }
                match folders.get(&key) {
                    Some(seen) if *seen != prefix => return Err(PathError::CaseCollision),
                    Some(_) => {}
                    None => {
                        folders.insert(key, prefix);
                    }
                }
            } else {
                if files.contains(&key) || folders.contains_key(&key) {
                    return Err(PathError::CaseCollision);
                }
                files.insert(key);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_relative_paths() {
        for ok in ["theme.json", "assets/icon.png", "a/b/c/d.txt", "My-File_1.0.wasm", "x"] {
            assert_eq!(validate_path(ok), Ok(()), "{ok}");
        }
    }

    #[test]
    fn refuses_escape_and_absolute_forms() {
        assert_eq!(validate_path(""), Err(PathError::Empty));
        assert_eq!(validate_path("/etc/passwd"), Err(PathError::EmptySegment));
        assert_eq!(validate_path("a//b"), Err(PathError::EmptySegment));
        assert_eq!(validate_path("a/"), Err(PathError::EmptySegment));
        assert_eq!(validate_path("../x"), Err(PathError::DotSegment));
        assert_eq!(validate_path("a/../x"), Err(PathError::DotSegment));
        assert_eq!(validate_path("./x"), Err(PathError::DotSegment));
        assert_eq!(validate_path("a/./x"), Err(PathError::DotSegment));
    }

    #[test]
    fn refuses_windows_and_odd_characters() {
        for bad in ["a\\b", "C:/x", "C:x", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b", "a b", "a\0b", "é.txt", "a\tb", "~x%41", "a\u{202e}b"] {
            assert_eq!(validate_path(bad), Err(PathError::BadCharacter), "{bad:?}");
        }
    }

    #[test]
    fn refuses_trailing_dots_and_reserved_device_names() {
        assert_eq!(validate_path("a."), Err(PathError::TrailingDot));
        assert_eq!(validate_path("dir./x"), Err(PathError::TrailingDot));
        for bad in ["con", "CON", "nul.txt", "dir/aux", "PRN.x", "com1", "LPT9.wasm", "Com5.tar.gz"] {
            assert_eq!(validate_path(bad), Err(PathError::ReservedName), "{bad}");
        }
        assert_eq!(validate_path("console.js"), Ok(()));
        assert_eq!(validate_path("com10"), Ok(()));
    }

    #[test]
    fn limits_length_segments_and_depth() {
        assert_eq!(validate_path(&"a".repeat(65)), Err(PathError::TooLong));
        assert_eq!(validate_path(&"a".repeat(64)), Ok(()));
        let long = format!("{}/{}/{}/{}", "a".repeat(60), "b".repeat(60), "c".repeat(60), "d".repeat(60));
        assert_eq!(validate_path(&long), Err(PathError::TooLong));
        assert_eq!(validate_path("a/b/c/d/e/f/g/h"), Ok(()));
        assert_eq!(validate_path("a/b/c/d/e/f/g/h/i"), Err(PathError::TooDeep));
    }

    #[test]
    fn whole_list_refuses_duplicates_and_case_collisions() {
        assert_eq!(validate_all(["a.txt", "b.txt"]), Ok(()));
        assert_eq!(validate_all(["a.txt", "a.txt"]), Err(PathError::Duplicate));
        assert_eq!(validate_all(["Readme.md", "README.md"]), Err(PathError::CaseCollision));
        assert_eq!(validate_all(["Dir/a", "dir/b"]), Err(PathError::CaseCollision));
        assert_eq!(validate_all(["a", "a/b"]), Err(PathError::CaseCollision));
        assert_eq!(validate_all(["dir/a", "dir/b", "other/a"]), Ok(()));
        assert_eq!(validate_all(["ok", "../bad"]), Err(PathError::DotSegment));
    }
}
