#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RenderEngineSceneToVideoArgs {
  #[serde(rename = "c")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub camera_animation: Option<String>,

  #[serde(rename = "s")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub camera_speed: Option<f32>,

  #[serde(rename = "b")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub skybox: Option<String>,
}
