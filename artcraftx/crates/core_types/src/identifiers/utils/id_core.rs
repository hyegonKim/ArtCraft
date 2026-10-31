//! The id *machinery*: minting, validation, and the [`define_id!`] macro.
//! Concrete `{Entity}Id` newtypes are declared in `lib.rs`, one
//! `define_id!` line each.

use std::fmt;

/// Mint a fresh id string: `{prefix}_{26-char lowercase Crockford ULID}`.
pub fn mint(prefix: &str) -> String {
  format!("{prefix}_{}", ulid::Ulid::new().to_string().to_lowercase())
}

/// Error parsing a prefixed id from an untrusted string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseIdError {
  /// The string didn't start with `{prefix}_`.
  WrongPrefix { expected: &'static str, got: String },
  /// The part after the prefix wasn't a 26-char Crockford ULID.
  BadUlid { got: String },
}

impl fmt::Display for ParseIdError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::WrongPrefix { expected, got } => {
        write!(f, "expected id with prefix '{expected}_', got '{got}'")
      }
      Self::BadUlid { got } => write!(f, "id '{got}' does not end in a 26-char ULID"),
    }
  }
}

impl std::error::Error for ParseIdError {}

/// Validate that `s` is a well-formed `{prefix}_{ulid}` id.
pub fn validate(s: &str, prefix: &'static str) -> Result<(), ParseIdError> {
  let rest = s
    .strip_prefix(prefix)
    .and_then(|r| r.strip_prefix('_'))
    .ok_or_else(|| ParseIdError::WrongPrefix { expected: prefix, got: s.to_string() })?;

  // A Crockford ULID is 26 chars of lowercase base32 (0-9 a-z after lowercasing).
  if rest.len() != 26 || !rest.bytes().all(|b| b.is_ascii_alphanumeric()) {
    return Err(ParseIdError::BadUlid { got: s.to_string() });
  }
  Ok(())
}

/// Define a strongly-typed id newtype: `define_id!(pub struct CredentialId => "credential");`.
///
/// The generated type is a transparent newtype over `String` that:
///   - `generate()`s a fresh `{prefix}_{ulid}`,
///   - parses/validates via `FromStr`,
///   - `Display`s / `Serialize`s as the bare string.
#[macro_export]
macro_rules! define_id {
  ($(#[$doc:meta])* $vis:vis struct $name:ident => $prefix:literal) => {
    $(#[$doc])*
    #[derive(
      Clone, PartialEq, Eq, PartialOrd, Ord, Hash,
      ::serde::Serialize, ::serde::Deserialize,
    )]
    #[serde(transparent)]
    $vis struct $name(String);

    impl $name {
      /// The id prefix (e.g. `credential`).
      pub const PREFIX: &'static str = $prefix;

      /// Generate a fresh, time-ordered id.
      pub fn generate() -> Self {
        Self($crate::identifiers::utils::id_core::mint($prefix))
      }

      /// Borrow the underlying string.
      pub fn as_str(&self) -> &str {
        &self.0
      }

      /// Consume into the underlying string.
      pub fn into_string(self) -> String {
        self.0
      }

      /// Wrap a string already known to be valid (e.g. one we minted and are
      /// round-tripping). Skips validation — use [`std::str::FromStr`] for
      /// untrusted input.
      pub fn from_trusted(s: impl Into<String>) -> Self {
        Self(s.into())
      }
    }

    impl ::std::str::FromStr for $name {
      type Err = $crate::identifiers::utils::id_core::ParseIdError;
      fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
        $crate::identifiers::utils::id_core::validate(s, $prefix)?;
        Ok(Self(s.to_string()))
      }
    }

    impl ::std::fmt::Display for $name {
      fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        f.write_str(&self.0)
      }
    }

    impl ::std::fmt::Debug for $name {
      fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        write!(f, concat!(stringify!($name), "({})"), &self.0)
      }
    }

    impl ::std::convert::AsRef<str> for $name {
      fn as_ref(&self) -> &str {
        &self.0
      }
    }
  };
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn mint_has_prefix_and_26_char_ulid() {
    let id = mint("credential");
    assert!(id.starts_with("credential_"));
    let (_, ulid) = id.split_once('_').unwrap();
    assert_eq!(ulid.len(), 26);
    assert_eq!(ulid, ulid.to_lowercase());
    assert!(validate(&id, "credential").is_ok());
  }

  #[test]
  fn validate_rejects_wrong_prefix() {
    let id = mint("acct");
    assert!(matches!(validate(&id, "credential"), Err(ParseIdError::WrongPrefix { .. })));
  }

  #[test]
  fn validate_rejects_bad_ulid() {
    assert!(matches!(validate("credential_short", "credential"), Err(ParseIdError::BadUlid { .. })));
    assert!(matches!(validate("credential_", "credential"), Err(ParseIdError::BadUlid { .. })));
  }
}
