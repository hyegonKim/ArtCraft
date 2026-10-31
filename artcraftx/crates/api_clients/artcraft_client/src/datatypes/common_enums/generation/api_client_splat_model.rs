use serde_derive::{Deserialize, Serialize};

/// Forward-compatible, self-contained API-client copy of the server's `CommonSplatModel` enum.
///
/// Serialized as a string. Any value this client build does not recognize is preserved
/// verbatim in [`Unknown`], so newer server variants never break deserialization and
/// still round-trip back to the original string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum ApiClientSplatModel {
  Marble0p1Mini,
  Marble0p1Plus,
  Marble1p0,
  Marble1p0Draft,
  Marble1p1,
  Marble1p1Plus,
  TripoSplat,
  /// A value not known to this client build (preserved as-is).
  Unknown(String),
}

impl From<String> for ApiClientSplatModel {
  fn from(value: String) -> Self {
    match value.as_str() {
      "marble_0p1_mini" => Self::Marble0p1Mini,
      "marble_0p1_plus" => Self::Marble0p1Plus,
      "marble_1p0" => Self::Marble1p0,
      "marble_1p0_draft" => Self::Marble1p0Draft,
      "marble_1p1" => Self::Marble1p1,
      "marble_1p1_plus" => Self::Marble1p1Plus,
      "triposplat" => Self::TripoSplat,
      _ => Self::Unknown(value),
    }
  }
}

impl From<ApiClientSplatModel> for String {
  fn from(value: ApiClientSplatModel) -> Self {
    match value {
      ApiClientSplatModel::Marble0p1Mini => "marble_0p1_mini".to_string(),
      ApiClientSplatModel::Marble0p1Plus => "marble_0p1_plus".to_string(),
      ApiClientSplatModel::Marble1p0 => "marble_1p0".to_string(),
      ApiClientSplatModel::Marble1p0Draft => "marble_1p0_draft".to_string(),
      ApiClientSplatModel::Marble1p1 => "marble_1p1".to_string(),
      ApiClientSplatModel::Marble1p1Plus => "marble_1p1_plus".to_string(),
      ApiClientSplatModel::TripoSplat => "triposplat".to_string(),
      ApiClientSplatModel::Unknown(other) => other,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn known_variant_round_trips() {
    let parsed: ApiClientSplatModel = serde_json::from_str("\"marble_0p1_mini\"").unwrap();
    assert_eq!(parsed, ApiClientSplatModel::Marble0p1Mini);
    assert_eq!(serde_json::to_string(&parsed).unwrap(), "\"marble_0p1_mini\"");
  }

  #[test]
  fn unknown_value_is_preserved_and_round_trips() {
    let parsed: ApiClientSplatModel = serde_json::from_str("\"zzz_not_a_real_value\"").unwrap();
    assert_eq!(parsed, ApiClientSplatModel::Unknown("zzz_not_a_real_value".to_string()));
    assert_eq!(serde_json::to_string(&parsed).unwrap(), "\"zzz_not_a_real_value\"");
  }

  #[test]
  fn every_known_variant_round_trips() {
    let all = [
      (ApiClientSplatModel::Marble0p1Mini, "marble_0p1_mini"),
      (ApiClientSplatModel::Marble0p1Plus, "marble_0p1_plus"),
      (ApiClientSplatModel::Marble1p0, "marble_1p0"),
      (ApiClientSplatModel::Marble1p0Draft, "marble_1p0_draft"),
      (ApiClientSplatModel::Marble1p1, "marble_1p1"),
      (ApiClientSplatModel::Marble1p1Plus, "marble_1p1_plus"),
      (ApiClientSplatModel::TripoSplat, "triposplat")
    ];
    for (variant, wire) in all {
      let json = format!("\"{}\"", wire);
      let parsed: ApiClientSplatModel = serde_json::from_str(&json).unwrap();
      assert_eq!(parsed, variant);
      assert_eq!(serde_json::to_string(&variant).unwrap(), json);
    }
  }
}
