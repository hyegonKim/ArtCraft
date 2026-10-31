use crate::types::string_enum::string_enum;

string_enum! {
  /// The `mode` quality tier some video pipelines take. Kling 3.0 maps its
  /// resolution menu onto it (720p → `std`, 1080p → `pro`, 4K → `4k`);
  /// Seedance 2.0 sends `std`.
  VideoMode {
    Std => "std",
    Pro => "pro",
    FourK => "4k",
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wire_round_trip() {
    for variant in VideoMode::known_variants() {
      assert_eq!(&VideoMode::from_str_lossy(variant.as_str()), variant);
    }
  }
}
