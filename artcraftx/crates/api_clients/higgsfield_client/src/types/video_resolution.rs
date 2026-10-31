use crate::types::string_enum::string_enum;

string_enum! {
  /// The `resolution` vocabulary across video models. Note the casing is
  /// the API's: `4k` lower-case (Seedance 2.0) but `2K` upper-case
  /// (MiniMax). Each model's request type narrows this to its own list.
  VideoResolution {
    P480 => "480p",
    P720 => "720p",
    P1080 => "1080p",
    TwoK => "2K",
    FourK => "4k",
  }
}

impl VideoResolution {
  /// The short side in pixels for a 16:9 frame at this tier (what the web
  /// app derives `width`/`height` from).
  pub fn short_side_px(&self) -> Option<u32> {
    match self {
      Self::P480 => Some(480),
      Self::P720 => Some(720),
      Self::P1080 => Some(1080),
      Self::TwoK => Some(1440),
      Self::FourK => Some(2160),
      Self::Other(_) => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wire_round_trip() {
    for variant in VideoResolution::known_variants() {
      assert_eq!(&VideoResolution::from_str_lossy(variant.as_str()), variant);
    }
    assert_eq!(serde_json::to_string(&VideoResolution::FourK).unwrap(), "\"4k\"");
    assert_eq!(serde_json::to_string(&VideoResolution::TwoK).unwrap(), "\"2K\"");
  }
}
