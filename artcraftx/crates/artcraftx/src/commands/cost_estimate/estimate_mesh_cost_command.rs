use artcraft_client::utils::api_host::ApiHost;
use crate::commands::utils::response::failure_response_wrapper::{CommandErrorResponseWrapper, CommandErrorStatus};
use crate::commands::utils::response::shorthand::ResponseOrError;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;
use artcraft_client::api_defs::omni_gen::cost_and_generate_requests::omni_gen_mesh_cost_and_generate_request::OmniGenMeshCostAndGenerateRequest;
use artcraft_client::api_defs::omni_gen::cost_response::omni_gen_mesh_cost_response::OmniGenMeshCostResponse;
use artcraft_client::endpoints::omni_gen::cost::mesh::omni_gen_mesh_cost::{omni_gen_mesh_cost, OmniGenMeshCostArgs};
use log::debug;
use serde_derive::Serialize;

impl SerializeMarker for OmniGenMeshCostResponse {}

#[derive(Serialize)]
pub struct EstimateMeshCostError {
  pub success: bool,
  pub error_message: String,
}

/// Estimate the cost of a mesh generation via the omni cost endpoint.
/// Anonymous: no credentials are needed for a baseline estimate.
#[tauri::command]
pub async fn estimate_mesh_cost_command(
  request: OmniGenMeshCostAndGenerateRequest,
) -> ResponseOrError<OmniGenMeshCostResponse, EstimateMeshCostError> {
  debug!("estimate_mesh_cost_command called");

  let result = omni_gen_mesh_cost(OmniGenMeshCostArgs {
    api_host: &ApiHost::Storyteller,
    api_or_web_creds: None,
    request: &request,
  }).await;

  match result {
    Ok(response) => Ok(response.into()),
    Err(err) => Err(CommandErrorResponseWrapper {
      status: CommandErrorStatus::BadRequest,
      error_message: None,
      error_type: None,
      error_details: Some(EstimateMeshCostError {
        success: false,
        error_message: err.to_string(),
      }),
    }),
  }
}
