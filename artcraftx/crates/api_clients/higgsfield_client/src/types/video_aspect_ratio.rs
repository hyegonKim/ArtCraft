use crate::types::image_aspect_ratio::ImageAspectRatio;
use serde::Serialize;

/// The aspect ratios the Seedance video models offer (2.5, 2.0, 2.0 Mini),
/// in menu order. `Auto` is on 2.0 / 2.0 Mini only (see each request type's
/// validation) and, for text-to-video, the web app sends `16:9` for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SeedanceVideoAspectRatio {
  Auto,
  Landscape21x9,
  Landscape16x9,
  Landscape4x3,
  Square1x1,
  Portrait3x4,
  Portrait9x16,
}

impl SeedanceVideoAspectRatio {
  pub fn all() -> [Self; 7] {
    [Self::Auto, Self::Landscape21x9, Self::Landscape16x9, Self::Landscape4x3, Self::Square1x1, Self::Portrait3x4, Self::Portrait9x16]
  }

  pub fn to_image_aspect_ratio(self) -> ImageAspectRatio {
    match self {
      Self::Auto => ImageAspectRatio::Auto,
      Self::Landscape21x9 => ImageAspectRatio::Landscape21x9,
      Self::Landscape16x9 => ImageAspectRatio::Landscape16x9,
      Self::Landscape4x3 => ImageAspectRatio::Landscape4x3,
      Self::Square1x1 => ImageAspectRatio::Square1x1,
      Self::Portrait3x4 => ImageAspectRatio::Portrait3x4,
      Self::Portrait9x16 => ImageAspectRatio::Portrait9x16,
    }
  }

  pub fn as_str(self) -> &'static str {
    match self {
      Self::Auto => "auto",
      Self::Landscape21x9 => "21:9",
      Self::Landscape16x9 => "16:9",
      Self::Landscape4x3 => "4:3",
      Self::Square1x1 => "1:1",
      Self::Portrait3x4 => "3:4",
      Self::Portrait9x16 => "9:16",
    }
  }
}

impl Serialize for SeedanceVideoAspectRatio {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.as_str())
  }
}

/// The aspect ratios Kling 3.0 offers, in menu order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum KlingAspectRatio {
  #[default]
  Landscape16x9,
  Portrait9x16,
  Square1x1,
}

impl KlingAspectRatio {
  pub fn all() -> [Self; 3] {
    [Self::Landscape16x9, Self::Portrait9x16, Self::Square1x1]
  }

  pub fn to_image_aspect_ratio(self) -> ImageAspectRatio {
    match self {
      Self::Landscape16x9 => ImageAspectRatio::Landscape16x9,
      Self::Portrait9x16 => ImageAspectRatio::Portrait9x16,
      Self::Square1x1 => ImageAspectRatio::Square1x1,
    }
  }

  pub fn as_str(self) -> &'static str {
    match self {
      Self::Landscape16x9 => "16:9",
      Self::Portrait9x16 => "9:16",
      Self::Square1x1 => "1:1",
    }
  }
}

impl Serialize for KlingAspectRatio {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.as_str())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn seedance_matches_the_web_app_menu() {
    let wire: Vec<&str> = SeedanceVideoAspectRatio::all().iter().map(|a| a.as_str()).collect();
    assert_eq!(wire, ["auto", "21:9", "16:9", "4:3", "1:1", "3:4", "9:16"]);
    for ratio in SeedanceVideoAspectRatio::all() {
      assert_eq!(ratio.to_image_aspect_ratio().as_str(), ratio.as_str());
    }
  }

  #[test]
  fn kling_matches_the_web_app_menu() {
    let wire: Vec<&str> = KlingAspectRatio::all().iter().map(|a| a.as_str()).collect();
    assert_eq!(wire, ["16:9", "9:16", "1:1"]);
  }
}
