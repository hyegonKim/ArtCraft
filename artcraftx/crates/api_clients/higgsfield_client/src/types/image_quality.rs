use crate::types::string_enum::string_enum;

string_enum! {
  /// The `quality` vocabulary across image models, as echoed in job params.
  /// GPT Image 2 uses low / medium / high; the Seedream models map their
  /// resolution menu onto basic / high / ultra. Each model's request type
  /// narrows this to its own list.
  ImageQuality {
    Low => "low",
    Medium => "medium",
    High => "high",
    Basic => "basic",
    Ultra => "ultra",
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wire_round_trip() {
    for variant in ImageQuality::known_variants() {
      assert_eq!(&ImageQuality::from_str_lossy(variant.as_str()), variant);
    }
    assert_eq!(serde_json::to_string(&ImageQuality::Ultra).unwrap(), "\"ultra\"");
  }
}
