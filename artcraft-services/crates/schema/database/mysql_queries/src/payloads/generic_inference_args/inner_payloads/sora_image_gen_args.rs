use tokens::tokens::media_files::MediaFileToken;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SoraImageGenArgs {
  #[serde(rename = "p")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub prompt: Option<String>,

  #[serde(rename = "sm")]
  pub scene_snapshot_media_token: Option<MediaFileToken>,

  #[serde(rename = "mf")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_additional_media_file_tokens: Option<Vec<MediaFileToken>>,

  #[serde(rename = "sa")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_number_of_samples: Option<u32>,

  #[serde(rename = "su")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_sora_media_upload_tokens: Option<Vec<String>>,

  #[serde(rename = "st")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_sora_task_id: Option<String>,
}
