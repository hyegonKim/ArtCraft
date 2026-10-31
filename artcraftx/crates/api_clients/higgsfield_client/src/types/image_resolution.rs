use crate::types::string_enum::string_enum;

string_enum! {
  /// Output resolution tier for image generation.
  ImageResolution {
    OneK => "1k",
    TwoK => "2k",
    FourK => "4k",
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wire_round_trip() {
    for variant in ImageResolution::known_variants() {
      assert_eq!(&ImageResolution::from_str_lossy(variant.as_str()), variant);
    }
    assert_eq!(serde_json::to_string(&ImageResolution::FourK).unwrap(), "\"4k\"");
  }
}
