use crate::types::string_enum::string_enum;

string_enum! {
  /// Seedance's output bitrate tier ("High — less compression, larger size"
  /// / "Standard — more compression, smaller size").
  ///
  /// NB: only `high` has been seen on the wire; `standard` is inferred from
  /// the menu label.
  VideoBitrateMode {
    High => "high",
    Standard => "standard",
  }
}

impl Default for VideoBitrateMode {
  fn default() -> Self {
    Self::High
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wire_round_trip() {
    assert_eq!(serde_json::to_string(&VideoBitrateMode::High).unwrap(), "\"high\"");
    assert_eq!(VideoBitrateMode::from_str_lossy("standard"), VideoBitrateMode::Standard);
    assert_eq!(VideoBitrateMode::default(), VideoBitrateMode::High);
  }
}
