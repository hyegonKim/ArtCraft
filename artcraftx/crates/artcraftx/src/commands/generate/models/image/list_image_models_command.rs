//! Tauri command: list every image model, with capabilities, presentation, and
//! which providers offer it. Served from the built-in `models` crate tables.

use log::info;
use models::configs::image_model_config::ImageModelConfig;
use models::configs::image_models::IMAGE_MODELS;
use models::providers::image_providers::{ImageProviderOffering, IMAGE_PROVIDERS};
use serde_derive::Serialize;

use crate::commands::utils::response::shorthand::ResponseOrErrorMessage;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;

#[derive(Clone, Debug, Serialize)]
pub struct ListImageModelsResponse {
  /// Every model, in picker order. Includes disabled models (`is_disabled`),
  /// which the frontend hides but keeps addressable.
  pub models: Vec<ImageModelConfig>,
  /// Which providers offer which of those models (a model absent from every
  /// provider can't be generated). The first provider listing a model is its
  /// default.
  pub providers: Vec<ImageProviderOffering>,
}

impl SerializeMarker for ListImageModelsResponse {}

#[tauri::command]
pub async fn list_image_models_command() -> ResponseOrErrorMessage<ListImageModelsResponse> {
  info!("list_image_models_command called");
  Ok(ListImageModelsResponse {
    models: IMAGE_MODELS.clone(),
    providers: IMAGE_PROVIDERS.clone(),
  }.into())
}
