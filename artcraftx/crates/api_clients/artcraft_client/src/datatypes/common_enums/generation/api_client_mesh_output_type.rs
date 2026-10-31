use serde_derive::{Deserialize, Serialize};

/// Forward-compatible, self-contained API-client copy of the server's `CommonMeshOutputType` enum.
///
/// Serialized as a string. Any value this client build does not recognize is preserved
/// verbatim in [`Unknown`], so newer server variants never break deserialization and
/// still round-trip back to the original string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum ApiClientMeshOutputType {
  Normal,
  LowPoly,
  Geometry,
  /// A value not known to this client build (preserved as-is).
  Unknown(String),
}

impl From<String> for ApiClientMeshOutputType {
  fn from(value: String) -> Self {
    match value.as_str() {
      "normal" => Self::Normal,
      "low_poly" => Self::LowPoly,
      "geometry" => Self::Geometry,
      _ => Self::Unknown(value),
    }
  }
}

impl From<ApiClientMeshOutputType> for String {
  fn from(value: ApiClientMeshOutputType) -> Self {
    match value {
      ApiClientMeshOutputType::Normal => "normal".to_string(),
      ApiClientMeshOutputType::LowPoly => "low_poly".to_string(),
      ApiClientMeshOutputType::Geometry => "geometry".to_string(),
      ApiClientMeshOutputType::Unknown(other) => other,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn known_variant_round_trips() {
    let parsed: ApiClientMeshOutputType = serde_json::from_str("\"normal\"").unwrap();
    assert_eq!(parsed, ApiClientMeshOutputType::Normal);
    assert_eq!(serde_json::to_string(&parsed).unwrap(), "\"normal\"");
  }

  #[test]
  fn unknown_value_is_preserved_and_round_trips() {
    let parsed: ApiClientMeshOutputType = serde_json::from_str("\"zzz_not_a_real_value\"").unwrap();
    assert_eq!(parsed, ApiClientMeshOutputType::Unknown("zzz_not_a_real_value".to_string()));
    assert_eq!(serde_json::to_string(&parsed).unwrap(), "\"zzz_not_a_real_value\"");
  }

  #[test]
  fn every_known_variant_round_trips() {
    let all = [
      (ApiClientMeshOutputType::Normal, "normal"),
      (ApiClientMeshOutputType::LowPoly, "low_poly"),
      (ApiClientMeshOutputType::Geometry, "geometry")
    ];
    for (variant, wire) in all {
      let json = format!("\"{}\"", wire);
      let parsed: ApiClientMeshOutputType = serde_json::from_str(&json).unwrap();
      assert_eq!(parsed, variant);
      assert_eq!(serde_json::to_string(&variant).unwrap(), json);
    }
  }
}
