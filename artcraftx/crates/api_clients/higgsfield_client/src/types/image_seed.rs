use rand::Rng;
use serde::{Deserialize, Serialize};

/// The web app picks seeds in this range (six digits).
const MAX_RANDOM_SEED: u32 = 1_000_000;

/// A generation seed. The Seedream endpoints expect one on every request;
/// the web app sends a fresh random one unless the user pins it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ImageSeed(u32);

impl ImageSeed {
  pub fn new(seed: u32) -> Self {
    Self(seed)
  }

  /// A random seed in the web app's range.
  pub fn random() -> Self {
    Self(rand::rng().random_range(0..MAX_RANDOM_SEED))
  }

  pub fn value(self) -> u32 {
    self.0
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn serializes_as_number() {
    assert_eq!(serde_json::to_string(&ImageSeed::new(158368)).unwrap(), "158368");
    let parsed: ImageSeed = serde_json::from_str("12745").unwrap();
    assert_eq!(parsed.value(), 12745);
  }

  #[test]
  fn random_is_in_range() {
    for _ in 0..50 {
      assert!(ImageSeed::random().value() < MAX_RANDOM_SEED);
    }
  }
}
