use std::fmt;

/// Where a `local_files` row is in thumbnail generation. Stored as text in
/// `thumbnail_state`; NEVER change an existing value, only add new ones.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ThumbnailState {
  /// Never generated, or the file's content changed since.
  Pending,
  /// The cache files exist (see the path columns).
  Generated,
  /// Can't be thumbnailed (container/codec); never retried.
  Unsupported,
  /// Generation errored; retried until the attempt cap.
  Failed,
}

impl ThumbnailState {
  pub fn to_str(&self) -> &'static str {
    match self {
      Self::Pending => "pending",
      Self::Generated => "generated",
      Self::Unsupported => "unsupported",
      Self::Failed => "failed",
    }
  }

  /// Unknown values (a newer binary wrote them) read as `Pending` so the
  /// worker re-derives them rather than erroring the whole row set.
  pub fn from_str_lenient(value: &str) -> Self {
    match value {
      "generated" => Self::Generated,
      "unsupported" => Self::Unsupported,
      "failed" => Self::Failed,
      _ => Self::Pending,
    }
  }
}

impl fmt::Display for ThumbnailState {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.to_str())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn round_trip() {
    for state in [ThumbnailState::Pending, ThumbnailState::Generated, ThumbnailState::Unsupported, ThumbnailState::Failed] {
      assert_eq!(ThumbnailState::from_str_lenient(state.to_str()), state);
    }
    assert_eq!(ThumbnailState::from_str_lenient("something_new"), ThumbnailState::Pending);
  }
}
