//! Tauri command: list every splat model, with capabilities, presentation, and
//! which providers offer it. Served from the built-in `models` crate tables.

use log::info;
use models::configs::splat_model_config::SplatModelConfig;
use models::configs::splat_models::SPLAT_MODELS;
use models::providers::splat_providers::{SplatProviderOffering, SPLAT_PROVIDERS};
use serde_derive::Serialize;

use crate::commands::utils::response::shorthand::ResponseOrErrorMessage;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;

#[derive(Clone, Debug, Serialize)]
pub struct ListSplatModelsResponse {
  /// Every model, in picker order. Includes disabled models (`is_disabled`),
  /// which the frontend hides but keeps addressable.
  pub models: Vec<SplatModelConfig>,
  /// Which providers offer which of those models (a model absent from every
  /// provider can't be generated). The first provider listing a model is its
  /// default.
  pub providers: Vec<SplatProviderOffering>,
}

impl SerializeMarker for ListSplatModelsResponse {}

#[tauri::command]
pub async fn list_splat_models_command() -> ResponseOrErrorMessage<ListSplatModelsResponse> {
  info!("list_splat_models_command called");
  Ok(ListSplatModelsResponse {
    models: SPLAT_MODELS.clone(),
    providers: SPLAT_PROVIDERS.clone(),
  }.into())
}
