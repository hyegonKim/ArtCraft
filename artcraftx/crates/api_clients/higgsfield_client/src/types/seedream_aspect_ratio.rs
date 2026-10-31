use crate::types::image_aspect_ratio::ImageAspectRatio;
use serde::Serialize;

/// The aspect ratios the web app offers for every Seedream model
/// (5.0 Pro, 5.0 lite, 4.5), in its menu order. No Auto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SeedreamAspectRatio {
  Square1x1,
  Landscape4x3,
  Portrait3x4,
  Landscape16x9,
  Landscape21x9,
  Portrait9x16,
  Portrait2x3,
  Landscape3x2,
}

impl SeedreamAspectRatio {
  pub fn all() -> [Self; 8] {
    [
      Self::Square1x1, Self::Landscape4x3, Self::Portrait3x4, Self::Landscape16x9,
      Self::Landscape21x9, Self::Portrait9x16, Self::Portrait2x3, Self::Landscape3x2,
    ]
  }

  /// The wire vocabulary value.
  pub fn to_image_aspect_ratio(self) -> ImageAspectRatio {
    match self {
      Self::Square1x1 => ImageAspectRatio::Square1x1,
      Self::Landscape4x3 => ImageAspectRatio::Landscape4x3,
      Self::Portrait3x4 => ImageAspectRatio::Portrait3x4,
      Self::Landscape16x9 => ImageAspectRatio::Landscape16x9,
      Self::Landscape21x9 => ImageAspectRatio::Landscape21x9,
      Self::Portrait9x16 => ImageAspectRatio::Portrait9x16,
      Self::Portrait2x3 => ImageAspectRatio::Portrait2x3,
      Self::Landscape3x2 => ImageAspectRatio::Landscape3x2,
    }
  }

  pub fn as_str(self) -> &'static str {
    match self {
      Self::Square1x1 => "1:1",
      Self::Landscape4x3 => "4:3",
      Self::Portrait3x4 => "3:4",
      Self::Landscape16x9 => "16:9",
      Self::Landscape21x9 => "21:9",
      Self::Portrait9x16 => "9:16",
      Self::Portrait2x3 => "2:3",
      Self::Landscape3x2 => "3:2",
    }
  }
}

impl Serialize for SeedreamAspectRatio {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.as_str())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn matches_the_web_app_menu() {
    let wire: Vec<&str> = SeedreamAspectRatio::all().iter().map(|a| a.as_str()).collect();
    assert_eq!(wire, ["1:1", "4:3", "3:4", "16:9", "21:9", "9:16", "2:3", "3:2"]);
    for ratio in SeedreamAspectRatio::all() {
      assert_eq!(ratio.to_image_aspect_ratio().as_str(), ratio.as_str());
    }
  }
}
