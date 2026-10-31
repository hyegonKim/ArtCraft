use serde_derive::{Deserialize, Serialize};

/// Forward-compatible, self-contained API-client copy of the server's `CommonAudioModel` enum.
///
/// Serialized as a string. Any value this client build does not recognize is preserved
/// verbatim in [`Unknown`], so newer server variants never break deserialization and
/// still round-trip back to the original string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum ApiClientAudioModel {
  SunoMusic,
  SunoRemix,
  SunoSounds,
  SunoSample,
  SeedAudio1p0,
  /// A value not known to this client build (preserved as-is).
  Unknown(String),
}

impl From<String> for ApiClientAudioModel {
  fn from(value: String) -> Self {
    match value.as_str() {
      "suno_music" => Self::SunoMusic,
      "suno_remix" => Self::SunoRemix,
      "suno_sounds" => Self::SunoSounds,
      "suno_sample" => Self::SunoSample,
      "seed_audio_1p0" => Self::SeedAudio1p0,
      _ => Self::Unknown(value),
    }
  }
}

impl From<ApiClientAudioModel> for String {
  fn from(value: ApiClientAudioModel) -> Self {
    match value {
      ApiClientAudioModel::SunoMusic => "suno_music".to_string(),
      ApiClientAudioModel::SunoRemix => "suno_remix".to_string(),
      ApiClientAudioModel::SunoSounds => "suno_sounds".to_string(),
      ApiClientAudioModel::SunoSample => "suno_sample".to_string(),
      ApiClientAudioModel::SeedAudio1p0 => "seed_audio_1p0".to_string(),
      ApiClientAudioModel::Unknown(other) => other,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn known_variant_round_trips() {
    let parsed: ApiClientAudioModel = serde_json::from_str("\"suno_music\"").unwrap();
    assert_eq!(parsed, ApiClientAudioModel::SunoMusic);
    assert_eq!(serde_json::to_string(&parsed).unwrap(), "\"suno_music\"");
  }

  #[test]
  fn unknown_value_is_preserved_and_round_trips() {
    let parsed: ApiClientAudioModel = serde_json::from_str("\"zzz_not_a_real_value\"").unwrap();
    assert_eq!(parsed, ApiClientAudioModel::Unknown("zzz_not_a_real_value".to_string()));
    assert_eq!(serde_json::to_string(&parsed).unwrap(), "\"zzz_not_a_real_value\"");
  }

  #[test]
  fn every_known_variant_round_trips() {
    let all = [
      (ApiClientAudioModel::SunoMusic, "suno_music"),
      (ApiClientAudioModel::SunoRemix, "suno_remix"),
      (ApiClientAudioModel::SunoSounds, "suno_sounds"),
      (ApiClientAudioModel::SunoSample, "suno_sample"),
      (ApiClientAudioModel::SeedAudio1p0, "seed_audio_1p0")
    ];
    for (variant, wire) in all {
      let json = format!("\"{}\"", wire);
      let parsed: ApiClientAudioModel = serde_json::from_str(&json).unwrap();
      assert_eq!(parsed, variant);
      assert_eq!(serde_json::to_string(&variant).unwrap(), json);
    }
  }
}
