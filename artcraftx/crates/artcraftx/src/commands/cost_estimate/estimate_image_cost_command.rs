use artcraft_client::utils::api_host::ApiHost;
use crate::commands::utils::response::failure_response_wrapper::{CommandErrorResponseWrapper, CommandErrorStatus};
use crate::commands::utils::response::shorthand::ResponseOrError;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;
use artcraft_client::api_defs::generate::cost_estimate::estimate_image_cost::{
  EstimateImageCostError, EstimateImageCostErrorType, EstimateImageCostRequest,
  EstimateImageCostResponse,
};
use artcraft_client::endpoints::generate::cost_estimate::image::estimate_image_cost::estimate_image_cost;
use log::debug;

impl SerializeMarker for EstimateImageCostResponse {}

#[tauri::command]
pub async fn estimate_image_cost_command(
  request: EstimateImageCostRequest,
) -> ResponseOrError<EstimateImageCostResponse, EstimateImageCostError> {
  debug!("estimate_image_cost_command called: {:?}", request);

  let result = estimate_image_cost(
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
      error_details: Some(EstimateImageCostError {
        success: false,
        error_type: EstimateImageCostErrorType::InvalidInput,
        error_message: err.to_string(),
      }),
    }),
  }
}
