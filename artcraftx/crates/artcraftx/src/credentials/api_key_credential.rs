use chrono::{DateTime, Utc};
use serde_derive::{Deserialize, Serialize};

/// How many leading characters of an API key are safe to show in the UI.
pub const PRINTABLE_PREFIX_LEN: usize = 5;

/// An API key secret for a service.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ApiKeyCredential {
  /// Full API key.
  pub api_key: String,

  /// First few characters of the API key that we can show in the UI.
  /// Recomputed from `api_key` on load, so hand-edited files can omit it
  /// (and a stale value never survives a key change).
  #[serde(default)]
  pub printable_partial_prefix: String,

  /// Last time a request with this key failed (e.g. rejected as invalid).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub failed_at: Option<DateTime<Utc>>,

  /// Last time a request with this key succeeded.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub succeeded_at: Option<DateTime<Utc>>,
}

impl ApiKeyCredential {
  pub fn new(api_key: impl Into<String>) -> Self {
    let api_key = api_key.into();
    let printable_partial_prefix = printable_prefix(&api_key);
    Self {
      api_key,
      printable_partial_prefix,
      failed_at: None,
      succeeded_at: None,
    }
  }

  /// Recompute the printable prefix from the full key.
  pub fn normalize(&mut self) {
    self.printable_partial_prefix = printable_prefix(&self.api_key);
  }
}

fn printable_prefix(api_key: &str) -> String {
  api_key.chars().take(PRINTABLE_PREFIX_LEN).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn new_computes_printable_prefix() {
    let credential = ApiKeyCredential::new("sk-abcdef123456");
    assert_eq!(credential.printable_partial_prefix, "sk-ab");
  }

  #[test]
  fn short_keys_do_not_panic() {
    let credential = ApiKeyCredential::new("abc");
    assert_eq!(credential.printable_partial_prefix, "abc");
  }

  #[test]
  fn normalize_repairs_stale_prefix() {
    let mut credential = ApiKeyCredential::new("sk-abcdef123456");
    credential.api_key = "sk-zzzzzz".to_string();
    credential.normalize();
    assert_eq!(credential.printable_partial_prefix, "sk-zz");
  }
}
