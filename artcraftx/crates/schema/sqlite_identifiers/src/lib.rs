//! Strongly-typed identifiers and enums for the desktop app's local SQLite
//! tasks database — the types that cross the query boundary as keys or
//! stored enum strings. Extracted from the `enums` / `tokens` crates so the
//! task database has a small, database-free dependency.

#[macro_use] extern crate serde_derive;

// Crockford characters
// https://en.wikipedia.org/wiki/Base32#Crockford's_Base32
pub(crate) const CROCKFORD_LOWERCASE_CHARSET: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";
pub(crate) const CROCKFORD_MIXED_CASE_CHARSET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZabcdefghjkmnpqrstvwxyz";

/// Display + Debug via `to_str()` for string-backed enums.
#[macro_export]
macro_rules! impl_enum_display_and_debug_using_to_str {
  ($t:ident) => {
    impl std::fmt::Display for $t {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
      }
    }
    impl std::fmt::Debug for $t {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
      }
    }
  };
}

/// Constructors and accessors for string-backed token newtypes.
#[macro_export]
macro_rules! impl_string_token {
  ($t:ident) => {
    impl $t {
      #[inline]
      pub fn new(value: String) -> Self {
        $t(value)
      }

      #[inline]
      pub fn new_from_str(value: &str) -> Self {
        $t(value.to_string())
      }

      #[inline]
      pub fn as_str(&self) -> &str {
        &self.0
      }

      /// The part of the token after the prefix (best-effort: returns the
      /// whole string if the prefix doesn't match).
      #[inline]
      pub fn entropy_suffix(&self) -> &str {
        self.0.strip_prefix(Self::PREFIX).unwrap_or(&self.0)
      }
    }

    impl std::fmt::Display for $t {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
      }
    }
  };
}

/// `generate()` for prefixed Crockford tokens: `{prefix}{entropy}` where the
/// total length is `$total_string_length`. Matches the original `tokens`
/// crate generator (including the safe-entropy re-roll).
#[macro_export]
macro_rules! impl_crockford_generator {
  ($t:ident, $total_string_length:literal, $prefix:literal, $charset:expr) => {
    impl $t {
      pub const PREFIX: &'static str = $prefix;

      /// Constructor for a new token.
      #[inline]
      pub fn generate() -> Self {
        use rand::Rng;

        let mut rng = rand::thread_rng();

        let charset: &[u8] = $charset;
        let entropy_length = $total_string_length.saturating_sub($prefix.len());

        let mut entropy_part: String = (0..entropy_length)
          .map(|_| {
            let idx = rng.gen_range(0..charset.len());
            charset[idx] as char
          })
          .collect();

        let mut i = 0;
        while !$crate::safe_entropy::entropy_is_safe(&entropy_part) && i < 10 {
          i += 1;
          entropy_part = (0..entropy_length)
            .map(|_| {
              let idx = rng.gen_range(0..charset.len());
              charset[idx] as char
            })
            .collect();
        }

        $t(format!("{}{}", $prefix, entropy_part))
      }
    }
  };
}

pub mod enums;
pub mod ids;
pub(crate) mod safe_entropy;

#[cfg(test)]
pub(crate) mod test_helpers;
