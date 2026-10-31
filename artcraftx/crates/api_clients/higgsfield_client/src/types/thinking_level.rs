use crate::types::string_enum::string_enum;

string_enum! {
  /// How much "thinking" a model spends on a prompt (Nano Banana 2 Lite's
  /// quality menu: High / Minimal). Upper-case on the wire.
  ThinkingLevel {
    High => "HIGH",
    Minimal => "MINIMAL",
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wire_round_trip() {
    assert_eq!(serde_json::to_string(&ThinkingLevel::Minimal).unwrap(), "\"MINIMAL\"");
    assert_eq!(ThinkingLevel::from_str_lossy("HIGH"), ThinkingLevel::High);
  }
}
