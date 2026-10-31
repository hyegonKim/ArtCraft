//! Tauri command: list every mesh model, with capabilities, presentation, and
//! which providers offer it. Served from the built-in `models` crate tables.

use log::info;
use models::configs::mesh_model_config::MeshModelConfig;
use models::configs::mesh_models::MESH_MODELS;
use models::providers::mesh_providers::{MeshProviderOffering, MESH_PROVIDERS};
use serde_derive::Serialize;

use crate::commands::utils::response::shorthand::ResponseOrErrorMessage;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;

#[derive(Clone, Debug, Serialize)]
pub struct ListMeshModelsResponse {
  /// Every model, in picker order. Includes disabled models (`is_disabled`),
  /// which the frontend hides but keeps addressable.
  pub models: Vec<MeshModelConfig>,
  /// Which providers offer which of those models (a model absent from every
  /// provider can't be generated). The first provider listing a model is its
  /// default.
  pub providers: Vec<MeshProviderOffering>,
}

impl SerializeMarker for ListMeshModelsResponse {}

#[tauri::command]
pub async fn list_mesh_models_command() -> ResponseOrErrorMessage<ListMeshModelsResponse> {
  info!("list_mesh_models_command called");
  Ok(ListMeshModelsResponse {
    models: MESH_MODELS.clone(),
    providers: MESH_PROVIDERS.clone(),
  }.into())
}
