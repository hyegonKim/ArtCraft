
/// We used to write stable diffusion outputs with these values in
/// the `extra_file_modification_info` field.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StableDiffusionExtraInfo {
  // NB: If we decide to encode this information again in the future,
  // it's probably best to use different fields or an entirely different struct.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub prompt: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub cfg_scale: Option<u32>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub negative_prompt: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub lora_model_weight_token: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub lora_name: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub sampler: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub width: Option<u32>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub height: Option<u32>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub seed: Option<i64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub number_of_samples: Option<u32>,
}

#[cfg(test)]
mod tests {
  use crate::payloads::media_file_extra_info::inner_payloads::stable_diffusion_extra_info::StableDiffusionExtraInfo;
  use crate::payloads::media_file_extra_info::media_file_extra_info::MediaFileExtraInfo;

  #[test]
  fn wrapped_enum() {
    let payload = MediaFileExtraInfo::S(StableDiffusionExtraInfo {
      prompt: Some("foo bar baz".to_string()),
      ..Default::default()
    });

    let json = r#"{"S":{"prompt":"foo bar baz"}}"#;

    let serialized = payload.to_json_string().unwrap();
    assert_eq!(&serialized, json);

    let deserialized = MediaFileExtraInfo::from_json_str(&json).unwrap();
    assert_eq!(deserialized, payload);
  }

  #[test]
  fn unwrapped_enum() {
    let json = r#"{"prompt":"foo bar baz"}"#;
    let deserialized = MediaFileExtraInfo::from_json_str(&json).unwrap();

    let expected = MediaFileExtraInfo::S(StableDiffusionExtraInfo {
      prompt: Some("foo bar baz".to_string()),
      ..Default::default()
    });

    assert_eq!(deserialized, expected);
  }
}
