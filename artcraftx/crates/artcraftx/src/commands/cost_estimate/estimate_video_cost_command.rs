use artcraft_client::utils::api_host::ApiHost;
use crate::commands::utils::response::failure_response_wrapper::{CommandErrorResponseWrapper, CommandErrorStatus};
use crate::commands::utils::response::shorthand::ResponseOrError;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;
use artcraft_client::api_defs::generate::cost_estimate::estimate_video_cost::{
  EstimateVideoCostError, EstimateVideoCostErrorType, EstimateVideoCostRequest,
  EstimateVideoCostResponse,
};
use artcraft_client::endpoints::generate::cost_estimate::video::estimate_video_cost::estimate_video_cost;
use log::debug;

impl SerializeMarker for EstimateVideoCostResponse {}

#[tauri::command]
pub async fn estimate_video_cost_command(
  request: EstimateVideoCostRequest,
) -> ResponseOrError<EstimateVideoCostResponse, EstimateVideoCostError> {
  debug!("estimate_video_cost_command called: {:?}", request);

  let result = estimate_video_cost(
    &ApiHost::Storyteller,
    None, // Credentials are not required for this endpoint.
    request,
  )
  .await;

  match result {
    Ok(response) => Ok(response.into()),
    Err(err) => Err(CommandErrorResponseWrapper {
      status: CommandErrorStatus::BadRequest,
      error_message: None,
      error_type: None,
      error_details: Some(EstimateVideoCostError {
        success: false,
        error_type: EstimateVideoCostErrorType::InvalidInput,
        error_message: err.to_string(),
      }),
    }),
  }
}
