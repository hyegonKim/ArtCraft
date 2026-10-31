#[derive(Clone, Debug, Serialize, Deserialize)]

pub struct TTSArgs {
  #[serde(rename = "vt")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub voice_token: Option<String>, //  varchar 32

  #[serde(rename = "dt")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub dataset_token: Option<String>
}
