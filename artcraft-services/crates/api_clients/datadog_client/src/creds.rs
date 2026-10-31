/// Opaque wrapper around a Datadog API key. `Debug` is implemented manually
/// so the secret never leaks to logs.
#[derive(Clone)]
pub struct DatadogApiKey(String);

impl DatadogApiKey {
  /// Construct from a raw key. Surrounding whitespace (including newlines)
  /// is stripped — this protects against secrets that were base64-encoded
  /// via `echo "$KEY" | base64` (which appends a newline) and arrive in
  /// the env var with a trailing `\n`. HTTP header values can't contain
  /// newlines, so an un-trimmed key would silently break every request
  /// with a reqwest `Builder` error.
  pub fn new(key: impl Into<String>) -> Self {
    let s = key.into();
    let trimmed = s.trim();
    if trimmed.len() != s.len() {
      // Don't log the key itself, just that we cleaned it.
      log::warn!(
        "DatadogApiKey: trimmed {} surrounding whitespace byte(s) from input",
        s.len() - trimmed.len(),
      );
    }
    Self(trimmed.to_string())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl std::fmt::Debug for DatadogApiKey {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("DatadogApiKey(***)")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn trims_trailing_newline() {
    let key = DatadogApiKey::new("abc123\n");
    assert_eq!(key.as_str(), "abc123");
  }

  #[test]
  fn trims_surrounding_whitespace() {
    let key = DatadogApiKey::new("  abc123\r\n\t  ");
    assert_eq!(key.as_str(), "abc123");
  }

  #[test]
  fn leaves_clean_key_untouched() {
    let key = DatadogApiKey::new("abc123");
    assert_eq!(key.as_str(), "abc123");
  }
}
