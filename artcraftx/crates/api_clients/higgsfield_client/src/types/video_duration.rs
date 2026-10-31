use crate::error::higgsfield_client_error::HiggsfieldClientError;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// A clip length in whole seconds. Every video model exposes duration as a
/// one-second-step slider; each model's request type validates against its
/// own [`VideoDurationRange`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VideoDurationSeconds(u32);

impl VideoDurationSeconds {
  pub const fn new(seconds: u32) -> Self {
    Self(seconds)
  }

  pub fn seconds(self) -> u32 {
    self.0
  }
}

impl Display for VideoDurationSeconds {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}s", self.0)
  }
}

/// The inclusive slider range a model offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoDurationRange {
  pub min_seconds: u32,
  pub max_seconds: u32,
}

impl VideoDurationRange {
  pub const fn new(min_seconds: u32, max_seconds: u32) -> Self {
    Self { min_seconds, max_seconds }
  }

  /// The cheapest clip the model offers.
  pub fn shortest(&self) -> VideoDurationSeconds {
    VideoDurationSeconds(self.min_seconds)
  }

  pub fn contains(&self, duration: VideoDurationSeconds) -> bool {
    (self.min_seconds..=self.max_seconds).contains(&duration.0)
  }

  pub fn validate(&self, duration: VideoDurationSeconds) -> Result<(), HiggsfieldClientError> {
    if self.contains(duration) {
      Ok(())
    } else {
      Err(HiggsfieldClientError::InvalidRequest(format!(
        "duration must be between {}s and {}s, got {}", self.min_seconds, self.max_seconds, duration,
      )))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn range_validation() {
    let range = VideoDurationRange::new(4, 15);
    assert_eq!(range.shortest(), VideoDurationSeconds::new(4));
    assert!(range.validate(VideoDurationSeconds::new(4)).is_ok());
    assert!(range.validate(VideoDurationSeconds::new(15)).is_ok());
    assert!(matches!(range.validate(VideoDurationSeconds::new(3)), Err(HiggsfieldClientError::InvalidRequest(_))));
    assert!(matches!(range.validate(VideoDurationSeconds::new(16)), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn serializes_as_number() {
    assert_eq!(serde_json::to_string(&VideoDurationSeconds::new(4)).unwrap(), "4");
    assert_eq!(VideoDurationSeconds::new(5).to_string(), "5s");
  }
}
