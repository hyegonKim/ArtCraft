use artcraft_client::utils::api_host::ApiHost;
use crate::commands::utils::response::failure_response_wrapper::{CommandErrorResponseWrapper, CommandErrorStatus};
use crate::commands::utils::response::shorthand::ResponseOrError;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;
use artcraft_client::api_defs::omni_gen::cost_and_generate_requests::omni_gen_splat_cost_and_generate_request::OmniGenSplatCostAndGenerateRequest;
use artcraft_client::api_defs::omni_gen::cost_response::omni_gen_splat_cost_response::OmniGenSplatCostResponse;
use artcraft_client::endpoints::omni_gen::cost::splat::omni_gen_splat_cost::{omni_gen_splat_cost, OmniGenSplatCostArgs};
use log::debug;
use serde_derive::Serialize;

impl SerializeMarker for OmniGenSplatCostResponse {}

#[derive(Serialize)]
pub struct EstimateSplatCostError {
  pub success: bool,
  pub error_message: String,
}

/// Estimate the cost of a splat generation via the omni cost endpoint.
/// (Migrated from the legacy `/v1/generate/cost_estimate/splat`, which does
/// not know the Marble 1.x models.)
/// Anonymous: no credentials are needed for a baseline estimate.
#[tauri::command]
pub async fn estimate_splat_cost_command(
  request: OmniGenSplatCostAndGenerateRequest,
) -> ResponseOrError<OmniGenSplatCostResponse, EstimateSplatCostError> {
  debug!("estimate_splat_cost_command called");

  let result = omni_gen_splat_cost(OmniGenSplatCostArgs {
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
      error_details: Some(EstimateSplatCostError {
        success: false,
        error_message: err.to_string(),
      }),
    }),
  }
}
