//! Tauri command: list every video model, with capabilities, presentation, and
//! which providers offer it. Served from the built-in `models` crate tables.

use log::info;
use models::configs::video_model_config::VideoModelConfig;
use models::configs::video_models::VIDEO_MODELS;
use models::providers::video_providers::{VideoProviderOffering, VIDEO_PROVIDERS};
use serde_derive::Serialize;

use crate::commands::utils::response::shorthand::ResponseOrErrorMessage;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;

#[derive(Clone, Debug, Serialize)]
pub struct ListVideoModelsResponse {
  /// Every model, in picker order. Includes disabled models (`is_disabled`),
  /// which the frontend hides but keeps addressable.
  pub models: Vec<VideoModelConfig>,
  /// Which providers offer which of those models (a model absent from every
  /// provider can't be generated). The first provider listing a model is its
  /// default.
  pub providers: Vec<VideoProviderOffering>,
}

impl SerializeMarker for ListVideoModelsResponse {}

#[tauri::command]
pub async fn list_video_models_command() -> ResponseOrErrorMessage<ListVideoModelsResponse> {
  info!("list_video_models_command called");
  Ok(ListVideoModelsResponse {
    models: VIDEO_MODELS.clone(),
    providers: VIDEO_PROVIDERS.clone(),
  }.into())
}
