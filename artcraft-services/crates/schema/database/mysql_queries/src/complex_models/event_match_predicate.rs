
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum EventMatchPredicate {
  /// Default value
  /// NB: This has an empty struct so it serializes as JSON and not a String!
  NotSet {},

  /// A predicate for an exactly matched cheermote name, including the bit value,
  /// eg "Cheer1". Case insensitive.
  BitsCheermoteNameExactMatch {
    cheermote_name: String,
  },

  /// A predicate for a cheermote name without value, but with a minimum bits
  /// threshold. eg "Cheer", minimum=150 bits. Case insensitive.
  /// Twitch also calls the leading part "prefix"
  BitsCheermotePrefixSpendThreshold {
    cheermote_prefix: String,
    minimum_bits_spent: u64,
  },

  /// Spend a minimum amount of bits.
  BitsSpendThreshold {
    minimum_bits_spent: u64,
  },

  ChannelPointsRewardNameExactMatch {
    reward_name: String,
  },
}

#[cfg(test)]
mod tests {
  use crate::complex_models::event_match_predicate::EventMatchPredicate;

  #[test]
  fn not_set() {
    let rust_value = EventMatchPredicate::NotSet {};
    let json = "{\"not_set\":{}}";

    let converted_to_json= serde_json::to_string(&rust_value).unwrap();
    assert_eq!(&converted_to_json, json);

    let converted_from_json : EventMatchPredicate = serde_json::from_str(json).unwrap();
    assert_eq!(&converted_from_json, &rust_value);
  }

  #[test]
  fn bits_cheermote_name_exact_match() {
    let rust_value = EventMatchPredicate::BitsCheermoteNameExactMatch {
      cheermote_name: "Cheer1".to_string()
    };
    let json = "{\"bits_cheermote_name_exact_match\":{\"cheermote_name\":\"Cheer1\"}}";

    let converted_to_json= serde_json::to_string(&rust_value).unwrap();
    assert_eq!(&converted_to_json, json);

    let converted_from_json : EventMatchPredicate = serde_json::from_str(json).unwrap();
    assert_eq!(&converted_from_json, &rust_value);
  }
}
