use artcraft_client::utils::api_host::ApiHost;
use crate::commands::utils::response::failure_response_wrapper::{CommandErrorResponseWrapper, CommandErrorStatus};
use crate::commands::utils::response::shorthand::ResponseOrError;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;
use artcraft_client::api_defs::omni_gen::cost_and_generate_requests::omni_gen_audio_cost_and_generate_request::OmniGenAudioCostAndGenerateRequest;
use artcraft_client::api_defs::omni_gen::cost_response::omni_gen_audio_cost_response::OmniGenAudioCostResponse;
use artcraft_client::endpoints::omni_gen::cost::audio::omni_gen_audio_cost::{omni_gen_audio_cost, OmniGenAudioCostArgs};
use log::debug;
use serde_derive::Serialize;

impl SerializeMarker for OmniGenAudioCostResponse {}

#[derive(Serialize)]
pub struct EstimateAudioCostError {
  pub success: bool,
  pub error_message: String,
}

/// Estimate the cost of a audio generation via the omni cost endpoint.
/// Anonymous: no credentials are needed for a baseline estimate.
#[tauri::command]
pub async fn estimate_audio_cost_command(
  request: OmniGenAudioCostAndGenerateRequest,
) -> ResponseOrError<OmniGenAudioCostResponse, EstimateAudioCostError> {
  debug!("estimate_audio_cost_command called");

  let result = omni_gen_audio_cost(OmniGenAudioCostArgs {
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
      error_details: Some(EstimateAudioCostError {
        success: false,
        error_message: err.to_string(),
      }),
    }),
  }
}
