use crate::types::image_aspect_ratio::ImageAspectRatio;
use crate::types::image_resolution::ImageResolution;
use serde::{Deserialize, Serialize};

/// Pixel dimensions the web app sends alongside aspect ratio + resolution.
///
/// The server treats these as advisory: it re-derives its own `width` /
/// `height` for the job (the enqueue response echoes the normalized values,
/// and they can change again once the job completes). We send what the web
/// app would so requests look the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageDimensions {
  pub width: u32,
  pub height: u32,
}

/// Dimensions snap to this grid on the short side, matching the web app.
const DIMENSION_STEP: u32 = 64;

impl ImageDimensions {
  pub fn new(width: u32, height: u32) -> Self {
    Self { width, height }
  }

  /// What the web app sends for an aspect ratio at a resolution tier: the
  /// long side is fixed per tier and the short side is derived from the
  /// ratio, rounded down to a multiple of 64.
  ///
  /// Long sides observed in captured requests: 1k → 1200, 2k → 2048,
  /// 4k → 5504. `Auto` is sent as a square. Returns `None` for an
  /// unparseable `Other` aspect ratio or an unknown resolution.
  pub fn for_aspect_ratio(aspect_ratio: &ImageAspectRatio, resolution: &ImageResolution) -> Option<Self> {
    let long_side = match resolution {
      ImageResolution::OneK => 1200,
      ImageResolution::TwoK => 2048,
      ImageResolution::FourK => 5504,
      ImageResolution::Other(_) => return None,
    };

    // "Auto" has no ratio of its own; the web app still has to send a size,
    // so use a square at the tier's long side and let the server pick.
    let ratio = match aspect_ratio {
      ImageAspectRatio::Auto => 1.0,
      other => other.ratio()?,
    };

    // ratio = width / height. The long side goes to whichever is bigger.
    let short_side_exact = if ratio >= 1.0 {
      long_side as f64 / ratio
    } else {
      long_side as f64 * ratio
    };
    let short_side = (short_side_exact as u32 / DIMENSION_STEP) * DIMENSION_STEP;

    if ratio >= 1.0 {
      Some(Self::new(long_side, short_side))
    } else {
      Some(Self::new(short_side, long_side))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // Values captured from the web app's own requests.

  #[test]
  fn matches_captured_nano_banana_3x4_1k() {
    let dims = ImageDimensions::for_aspect_ratio(&ImageAspectRatio::Portrait3x4, &ImageResolution::OneK).unwrap();
    assert_eq!(dims, ImageDimensions::new(896, 1200));
  }

  #[test]
  fn matches_captured_nano_banana_16x9_4k() {
    let dims = ImageDimensions::for_aspect_ratio(&ImageAspectRatio::Landscape16x9, &ImageResolution::FourK).unwrap();
    assert_eq!(dims, ImageDimensions::new(5504, 3072));
  }

  #[test]
  fn matches_captured_gpt_image_9x16_2k() {
    let dims = ImageDimensions::for_aspect_ratio(&ImageAspectRatio::Portrait9x16, &ImageResolution::TwoK).unwrap();
    assert_eq!(dims, ImageDimensions::new(1152, 2048));
  }

  #[test]
  fn square_uses_long_side_for_both() {
    let dims = ImageDimensions::for_aspect_ratio(&ImageAspectRatio::Square1x1, &ImageResolution::TwoK).unwrap();
    assert_eq!(dims, ImageDimensions::new(2048, 2048));
  }

  #[test]
  fn auto_is_sent_like_square() {
    for resolution in [ImageResolution::OneK, ImageResolution::TwoK, ImageResolution::FourK] {
      let auto = ImageDimensions::for_aspect_ratio(&ImageAspectRatio::Auto, &resolution).unwrap();
      let square = ImageDimensions::for_aspect_ratio(&ImageAspectRatio::Square1x1, &resolution).unwrap();
      assert_eq!(auto, square, "{}", resolution);
    }
  }

  #[test]
  fn unknown_resolution_or_ratio_is_none() {
    assert!(ImageDimensions::for_aspect_ratio(&ImageAspectRatio::Square1x1, &ImageResolution::Other("8k".into())).is_none());
    assert!(ImageDimensions::for_aspect_ratio(&ImageAspectRatio::Other("wide".into()), &ImageResolution::OneK).is_none());
  }
}
