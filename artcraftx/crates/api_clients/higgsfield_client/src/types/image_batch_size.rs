use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// How many images one enqueue produces. The web app's counter runs 1–4
/// for every image model, so this is a closed set rather than a bare
/// integer. Serializes as the number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ImageBatchSize {
  #[default]
  One,
  Two,
  Three,
  Four,
}

impl ImageBatchSize {
  pub const MIN: u32 = 1;
  pub const MAX: u32 = 4;

  pub fn as_u32(self) -> u32 {
    match self {
      Self::One => 1,
      Self::Two => 2,
      Self::Three => 3,
      Self::Four => 4,
    }
  }

  /// `None` outside 1–4.
  pub fn from_u32(count: u32) -> Option<Self> {
    match count {
      1 => Some(Self::One),
      2 => Some(Self::Two),
      3 => Some(Self::Three),
      4 => Some(Self::Four),
      _ => None,
    }
  }

  pub fn all() -> [Self; 4] {
    [Self::One, Self::Two, Self::Three, Self::Four]
  }
}

impl Display for ImageBatchSize {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.as_u32())
  }
}

impl Serialize for ImageBatchSize {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u32(self.as_u32())
  }
}

impl<'de> Deserialize<'de> for ImageBatchSize {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let count = u32::deserialize(deserializer)?;
    Self::from_u32(count).ok_or_else(|| serde::de::Error::custom(format!("batch size must be 1-4, got {count}")))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn round_trips_through_numbers() {
    for size in ImageBatchSize::all() {
      assert_eq!(ImageBatchSize::from_u32(size.as_u32()), Some(size));
      let json = serde_json::to_string(&size).unwrap();
      assert_eq!(json, size.as_u32().to_string());
      assert_eq!(serde_json::from_str::<ImageBatchSize>(&json).unwrap(), size);
    }
  }

  #[test]
  fn rejects_out_of_range() {
    assert_eq!(ImageBatchSize::from_u32(0), None);
    assert_eq!(ImageBatchSize::from_u32(5), None);
    assert!(serde_json::from_str::<ImageBatchSize>("7").is_err());
  }

  #[test]
  fn default_is_one() {
    assert_eq!(ImageBatchSize::default(), ImageBatchSize::One);
  }
}
