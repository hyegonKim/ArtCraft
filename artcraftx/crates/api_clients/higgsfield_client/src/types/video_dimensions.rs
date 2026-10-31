use crate::types::image_aspect_ratio::ImageAspectRatio;
use crate::types::video_resolution::VideoResolution;
use serde::{Deserialize, Serialize};

/// Pixel dimensions the web app sends alongside a video's aspect ratio and
/// resolution. The server treats them as advisory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoDimensions {
  pub width: u32,
  pub height: u32,
}

impl VideoDimensions {
  pub fn new(width: u32, height: u32) -> Self {
    Self { width, height }
  }

  /// What the web app sends: the resolution tier fixes the short side and
  /// the long side follows the aspect ratio, rounded to an even number.
  /// `Auto` (text-to-video, no reference frame) is sent as 16:9.
  ///
  /// Observed: 16:9 @ 480p → 854×480, 16:9 @ 720p → 1280×720,
  /// auto @ 2K → 2560×1440.
  pub fn for_aspect_ratio(aspect_ratio: &ImageAspectRatio, resolution: &VideoResolution) -> Option<Self> {
    let short_side = resolution.short_side_px()?;
    let ratio = match aspect_ratio {
      ImageAspectRatio::Auto => 16.0 / 9.0,
      other => other.ratio()?,
    };
    let round_even = |value: f64| -> u32 {
      let rounded = value.round() as u32;
      if rounded % 2 == 0 { rounded } else { rounded + 1 }
    };
    if ratio >= 1.0 {
      Some(Self::new(round_even(short_side as f64 * ratio), short_side))
    } else {
      Some(Self::new(short_side, round_even(short_side as f64 / ratio)))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn matches_captured_requests() {
    assert_eq!(VideoDimensions::for_aspect_ratio(&ImageAspectRatio::Landscape16x9, &VideoResolution::P480), Some(VideoDimensions::new(854, 480)));
    assert_eq!(VideoDimensions::for_aspect_ratio(&ImageAspectRatio::Landscape16x9, &VideoResolution::P720), Some(VideoDimensions::new(1280, 720)));
    assert_eq!(VideoDimensions::for_aspect_ratio(&ImageAspectRatio::Auto, &VideoResolution::TwoK), Some(VideoDimensions::new(2560, 1440)));
  }

  #[test]
  fn portrait_and_square() {
    assert_eq!(VideoDimensions::for_aspect_ratio(&ImageAspectRatio::Portrait9x16, &VideoResolution::P480), Some(VideoDimensions::new(480, 854)));
    assert_eq!(VideoDimensions::for_aspect_ratio(&ImageAspectRatio::Square1x1, &VideoResolution::P1080), Some(VideoDimensions::new(1080, 1080)));
  }

  #[test]
  fn unknown_inputs_are_none() {
    assert!(VideoDimensions::for_aspect_ratio(&ImageAspectRatio::Other("x".into()), &VideoResolution::P480).is_none());
    assert!(VideoDimensions::for_aspect_ratio(&ImageAspectRatio::Square1x1, &VideoResolution::Other("8k".into())).is_none());
  }
}
