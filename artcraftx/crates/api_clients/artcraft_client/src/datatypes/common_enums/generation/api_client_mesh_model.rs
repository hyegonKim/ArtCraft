use serde_derive::{Deserialize, Serialize};

/// Forward-compatible, self-contained API-client copy of the server's `CommonMeshModel` enum.
///
/// Serialized as a string. Any value this client build does not recognize is preserved
/// verbatim in [`Unknown`], so newer server variants never break deserialization and
/// still round-trip back to the original string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum ApiClientMeshModel {
  Hunyuan3d2p0,
  Hunyuan3d2p1,
  Hunyuan3d3,
  Hunyuan3d3Sketch,
  Hunyuan3d3p1Pro,
  Hunyuan3d3p1Rapid,
  Hunyuan3d3p1Part,
  Hunyuan3d3p1SmartTopology,
  Tripo3dH3p1,
  MeshyV6,
  Rodin2p5Fast,
  /// A value not known to this client build (preserved as-is).
  Unknown(String),
}

impl From<String> for ApiClientMeshModel {
  fn from(value: String) -> Self {
    match value.as_str() {
      "hunyuan_3d_2p0" => Self::Hunyuan3d2p0,
      "hunyuan_3d_2p1" => Self::Hunyuan3d2p1,
      "hunyuan_3d_3" => Self::Hunyuan3d3,
      "hunyuan_3d_3_sketch" => Self::Hunyuan3d3Sketch,
      "hunyuan_3d_3p1_pro" => Self::Hunyuan3d3p1Pro,
      "hunyuan_3d_3p1_rapid" => Self::Hunyuan3d3p1Rapid,
      "hunyuan_3d_3p1_part" => Self::Hunyuan3d3p1Part,
      "hunyuan_3d_3p1_topology" => Self::Hunyuan3d3p1SmartTopology,
      "tripo3d_h3p1" => Self::Tripo3dH3p1,
      "meshy_v6" => Self::MeshyV6,
      "rodin_2p5_fast" => Self::Rodin2p5Fast,
      _ => Self::Unknown(value),
    }
  }
}

impl From<ApiClientMeshModel> for String {
  fn from(value: ApiClientMeshModel) -> Self {
    match value {
      ApiClientMeshModel::Hunyuan3d2p0 => "hunyuan_3d_2p0".to_string(),
      ApiClientMeshModel::Hunyuan3d2p1 => "hunyuan_3d_2p1".to_string(),
      ApiClientMeshModel::Hunyuan3d3 => "hunyuan_3d_3".to_string(),
      ApiClientMeshModel::Hunyuan3d3Sketch => "hunyuan_3d_3_sketch".to_string(),
      ApiClientMeshModel::Hunyuan3d3p1Pro => "hunyuan_3d_3p1_pro".to_string(),
      ApiClientMeshModel::Hunyuan3d3p1Rapid => "hunyuan_3d_3p1_rapid".to_string(),
      ApiClientMeshModel::Hunyuan3d3p1Part => "hunyuan_3d_3p1_part".to_string(),
      ApiClientMeshModel::Hunyuan3d3p1SmartTopology => "hunyuan_3d_3p1_topology".to_string(),
      ApiClientMeshModel::Tripo3dH3p1 => "tripo3d_h3p1".to_string(),
      ApiClientMeshModel::MeshyV6 => "meshy_v6".to_string(),
      ApiClientMeshModel::Rodin2p5Fast => "rodin_2p5_fast".to_string(),
      ApiClientMeshModel::Unknown(other) => other,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn known_variant_round_trips() {
    let parsed: ApiClientMeshModel = serde_json::from_str("\"hunyuan_3d_2p0\"").unwrap();
    assert_eq!(parsed, ApiClientMeshModel::Hunyuan3d2p0);
    assert_eq!(serde_json::to_string(&parsed).unwrap(), "\"hunyuan_3d_2p0\"");
  }

  #[test]
  fn unknown_value_is_preserved_and_round_trips() {
    let parsed: ApiClientMeshModel = serde_json::from_str("\"zzz_not_a_real_value\"").unwrap();
    assert_eq!(parsed, ApiClientMeshModel::Unknown("zzz_not_a_real_value".to_string()));
    assert_eq!(serde_json::to_string(&parsed).unwrap(), "\"zzz_not_a_real_value\"");
  }

  #[test]
  fn every_known_variant_round_trips() {
    let all = [
      (ApiClientMeshModel::Hunyuan3d2p0, "hunyuan_3d_2p0"),
      (ApiClientMeshModel::Hunyuan3d2p1, "hunyuan_3d_2p1"),
      (ApiClientMeshModel::Hunyuan3d3, "hunyuan_3d_3"),
      (ApiClientMeshModel::Hunyuan3d3Sketch, "hunyuan_3d_3_sketch"),
      (ApiClientMeshModel::Hunyuan3d3p1Pro, "hunyuan_3d_3p1_pro"),
      (ApiClientMeshModel::Hunyuan3d3p1Rapid, "hunyuan_3d_3p1_rapid"),
      (ApiClientMeshModel::Hunyuan3d3p1Part, "hunyuan_3d_3p1_part"),
      (ApiClientMeshModel::Hunyuan3d3p1SmartTopology, "hunyuan_3d_3p1_topology"),
      (ApiClientMeshModel::Tripo3dH3p1, "tripo3d_h3p1"),
      (ApiClientMeshModel::MeshyV6, "meshy_v6"),
      (ApiClientMeshModel::Rodin2p5Fast, "rodin_2p5_fast")
    ];
    for (variant, wire) in all {
      let json = format!("\"{}\"", wire);
      let parsed: ApiClientMeshModel = serde_json::from_str(&json).unwrap();
      assert_eq!(parsed, variant);
      assert_eq!(serde_json::to_string(&variant).unwrap(), json);
    }
  }
}
