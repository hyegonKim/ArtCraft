use crate::types::image_aspect_ratio::ImageAspectRatio;
use serde::Serialize;

/// The aspect ratios the web app offers for the Nano Banana models (Pro, 2,
/// 2 Lite), in its menu order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NanoBananaAspectRatio {
  /// Let the model pick (meant for reference-image workflows).
  Auto,
  Square1x1,
  Portrait3x4,
  Landscape4x3,
  Portrait2x3,
  Landscape3x2,
  Portrait9x16,
  Landscape16x9,
  Landscape5x4,
  Portrait4x5,
  Landscape21x9,
}

impl NanoBananaAspectRatio {
  pub fn all() -> [Self; 11] {
    [
      Self::Auto, Self::Square1x1, Self::Portrait3x4, Self::Landscape4x3, Self::Portrait2x3, Self::Landscape3x2,
      Self::Portrait9x16, Self::Landscape16x9, Self::Landscape5x4, Self::Portrait4x5, Self::Landscape21x9,
    ]
  }

  /// The wire vocabulary value.
  pub fn to_image_aspect_ratio(self) -> ImageAspectRatio {
    match self {
      Self::Auto => ImageAspectRatio::Auto,
      Self::Square1x1 => ImageAspectRatio::Square1x1,
      Self::Portrait3x4 => ImageAspectRatio::Portrait3x4,
      Self::Landscape4x3 => ImageAspectRatio::Landscape4x3,
      Self::Portrait2x3 => ImageAspectRatio::Portrait2x3,
      Self::Landscape3x2 => ImageAspectRatio::Landscape3x2,
      Self::Portrait9x16 => ImageAspectRatio::Portrait9x16,
      Self::Landscape16x9 => ImageAspectRatio::Landscape16x9,
      Self::Landscape5x4 => ImageAspectRatio::Landscape5x4,
      Self::Portrait4x5 => ImageAspectRatio::Portrait4x5,
      Self::Landscape21x9 => ImageAspectRatio::Landscape21x9,
    }
  }

  pub fn as_str(self) -> &'static str {
    match self {
      Self::Auto => "auto",
      Self::Square1x1 => "1:1",
      Self::Portrait3x4 => "3:4",
      Self::Landscape4x3 => "4:3",
      Self::Portrait2x3 => "2:3",
      Self::Landscape3x2 => "3:2",
      Self::Portrait9x16 => "9:16",
      Self::Landscape16x9 => "16:9",
      Self::Landscape5x4 => "5:4",
      Self::Portrait4x5 => "4:5",
      Self::Landscape21x9 => "21:9",
    }
  }
}

impl Serialize for NanoBananaAspectRatio {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.as_str())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn matches_the_web_app_menu() {
    let wire: Vec<&str> = NanoBananaAspectRatio::all().iter().map(|a| a.as_str()).collect();
    assert_eq!(wire, ["auto", "1:1", "3:4", "4:3", "2:3", "3:2", "9:16", "16:9", "5:4", "4:5", "21:9"]);
    for ratio in NanoBananaAspectRatio::all() {
      assert_eq!(ratio.to_image_aspect_ratio().as_str(), ratio.as_str());
    }
  }
}
