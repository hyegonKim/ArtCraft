use crate::commands::generate::common::notify_frontend_of_errors::notify_frontend_of_errors;
use crate::commands::generate::generate_error::{GenerateError, MissingCredentialsReason};
use crate::commands::generate::task_enqueue_success::TaskEnqueueSuccess;
use crate::commands::generate::generate_image::enqueue_image_generation::enqueue_image_generation;
use crate::commands::generate::generate_image::tauri_generate_image_request::{
  TauriGenerateImageErrorType, TauriGenerateImageRequest, TauriGenerateImageResponse,
};
use crate::services::grok::state::grok_websockets::GrokWebsockets;
use crate::services::midjourney::state::midjourney_live_session::MidjourneyLiveSession;
use crate::commands::utils::response::failure_response_wrapper::{CommandErrorResponseWrapper, CommandErrorStatus};
use crate::commands::utils::response::shorthand::Response;
use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::functional_events::credits_balance_changed_event::CreditsBalanceChangedEvent;
use crate::events::generation_events::generation_enqueue_success_event::GenerationEnqueueSuccessEvent;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::database::local_files_database::LocalFilesDatabase;
use crate::services::asset_uploads::asset_upload_ledger::AssetUploadLedger;
use crate::state::database::task_database::TaskDatabase;
use log::{error, info};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn generate_image_command(
  request: TauriGenerateImageRequest,
  app: AppHandle,
  app_data_root: State<'_, AppDataRoot>,
  task_database: State<'_, TaskDatabase>,
  midjourney_live_session: State<'_, MidjourneyLiveSession>,
  grok_websockets: State<'_, GrokWebsockets>,
  local_files_database: State<'_, LocalFilesDatabase>,
) -> Response<TauriGenerateImageResponse, TauriGenerateImageErrorType, ()> {

  info!("generate_image_command called, request: {:?}", request);

  // Generation is credential-driven: the request names a stored credential,
  // and we invoke the router for that credential's service.
  let ledger = AssetUploadLedger::new(local_files_database.inner().clone());
  let result = enqueue_image_generation(
    &request,
    &app,
    &app_data_root,
    &midjourney_live_session,
    &grok_websockets,
    &ledger,
  ).await;

  match result {
    Ok(success) => handle_success_behavior(&app, &task_database, &request, success).await,
    Err(err) => handle_error_behavior(&app, err).await,
  }
}

// ── Result mapping ──

async fn handle_success_behavior(
  app: &AppHandle,
  task_database: &TaskDatabase,
  request: &TauriGenerateImageRequest,
  success: TaskEnqueueSuccess,
) -> Response<TauriGenerateImageResponse, TauriGenerateImageErrorType, ()> {


  // Insert into task database
  let db_result = success
      .insert_into_task_database_with_frontend_payload(
        &task_database,
        request.frontend_caller,
        request.frontend_subscriber_id.as_deref(),
        request.frontend_subscriber_payload.as_deref(),
      )
      .await;

  if let Err(err) = db_result {
    error!("Failed to create task in database: {:?}", err);
  }

  /*
  let usage_type = if is_image_to_image {
    ArtcraftUsageType::ImageToResult
  } else {
    ArtcraftUsageType::TextToResult
  };

  if let Err(err) = artcraft_usage_tracker.record_image_generation(num_images, usage_type, ArtcraftUsagePage::ImagePage) {
    // NB: Fail open.
    warn!("Failed to report usage: {:?}", err);
  }
   */

  let event = GenerationEnqueueSuccessEvent {
    action: success.to_frontend_event_action(),
    service: success.to_frontend_event_service(),
    model: success.model,
  };

  if let Err(err) = event.send(&app) {
    error!("Failed to emit event: {:?}", err);
  }

  CreditsBalanceChangedEvent{}.send_infallible(&app);

  Ok(TauriGenerateImageResponse {}.into())
}

async fn handle_error_behavior(
  app: &AppHandle,
  err: GenerateError,
) -> Response<TauriGenerateImageResponse, TauriGenerateImageErrorType, ()> {
  error!("generate_image_command error: {:?}", err);

  notify_frontend_of_errors(&app, &err).await;

  let error_type = match &err {
    GenerateError::BadInput(_) => TauriGenerateImageErrorType::BadInput,
    GenerateError::MissingCredentials(MissingCredentialsReason::NeedsFalApiKey) => TauriGenerateImageErrorType::NeedsFalApiKey,
    GenerateError::MissingCredentials(MissingCredentialsReason::NeedsGrokCredentials) => TauriGenerateImageErrorType::NeedsGrokCredentials,
    GenerateError::MissingCredentials(_) => TauriGenerateImageErrorType::NeedsStorytellerCredentials,
    GenerateError::CredentialProblem(_) => TauriGenerateImageErrorType::CredentialProblem,
    GenerateError::NoProviderAvailable => TauriGenerateImageErrorType::NoProviderAvailable,
    GenerateError::BillingIssue(_) => TauriGenerateImageErrorType::BillingIssue,
    _ => TauriGenerateImageErrorType::ServerError,
  };

  // TODO:
  /*
    let mut status = CommandErrorStatus::ServerError;
    let mut error_type = EnqueueTextToImageErrorType::ServerError;
    let mut error_message = "A server error occurred. Please try again. If it continues, please tell our staff about the problem.";

    match err {
      GenerateError::BadInput(BadInputReason::NoModelSpecified) => {
        status = CommandErrorStatus::BadRequest;
        error_type = EnqueueTextToImageErrorType::ModelNotSpecified;
        error_message = "No model specified for image generation";
      }
      GenerateError::NoProviderAvailable => {
        status = CommandErrorStatus::ServerError;
        error_type = EnqueueTextToImageErrorType::NoProviderAvailable;
        error_message = "No configured provider available for image generation";
      }
      GenerateError::MissingCredentials(MissingCredentialsReason::NeedsFalApiKey) => {
        status = CommandErrorStatus::Unauthorized;
        error_type = EnqueueTextToImageErrorType::NeedsFalApiKey;
        error_message = "You need to set a FAL api key";
      },
      _ => {}, // Fall-through
    }

    Err(CommandErrorResponseWrapper {
      status,
      error_message: Some(error_message.to_string()),
      error_type: Some(error_type),
      error_details: None,
    })
   */

  // Prefer a human-readable message for the frontend toast; fall back to the
  // debug rendering for error kinds without one.
  let error_message = match &err {
    GenerateError::NotYetImplemented(message) => message.clone(),
    GenerateError::ProviderRejected(message) => message.clone(),
    GenerateError::CredentialProblem(reason) => reason.user_message(),
    other => format!("{:?}", other),
  };

  Err(CommandErrorResponseWrapper {
    status: CommandErrorStatus::ServerError,
    error_message: Some(error_message),
    error_type: Some(error_type),
    error_details: None,
  })
}
