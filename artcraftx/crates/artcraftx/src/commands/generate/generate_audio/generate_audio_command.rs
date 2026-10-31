use crate::commands::generate::common::notify_frontend_of_errors::notify_frontend_of_errors;
use crate::commands::generate::generate_error::GenerateError;
use crate::commands::generate::task_enqueue_success::TaskEnqueueSuccess;
use crate::commands::generate::generate_audio::handle_credential_router::handle_credential_router;
use crate::commands::generate::generate_audio::request::{
  TauriGenerateAudioErrorType, TauriGenerateAudioRequest, TauriGenerateAudioResponse,
};
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
pub async fn generate_audio_command(
  request: TauriGenerateAudioRequest,
  app: AppHandle,
  app_data_root: State<'_, AppDataRoot>,
  task_database: State<'_, TaskDatabase>,
  local_files_database: State<'_, LocalFilesDatabase>,
) -> Response<TauriGenerateAudioResponse, TauriGenerateAudioErrorType, ()> {

  info!("generate_audio_command called, request: {:?}", request);

  let ledger = AssetUploadLedger::new(local_files_database.inner().clone());
  let result = handle_request(
    request,
    &app_data_root,
    &task_database,
    &ledger,
  ).await;

  match result {
    Err(err) => {
      error!("generate_audio_command error: {:?}", err);

      notify_frontend_of_errors(&app, &err).await;

      let mut status = CommandErrorStatus::ServerError;
      let mut error_type = TauriGenerateAudioErrorType::ServerError;
      let mut error_message = "A server error occurred. Please try again. If it continues, please tell our staff about the problem.".to_string();

      match err {
        GenerateError::BadInput(_) => {
          status = CommandErrorStatus::BadRequest;
          error_type = TauriGenerateAudioErrorType::ModelNotSpecified;
          error_message = "No model specified for audio generation".to_string();
        }
        GenerateError::CredentialProblem(reason) => {
          status = CommandErrorStatus::Unauthorized;
          error_type = TauriGenerateAudioErrorType::CredentialProblem;
          error_message = reason.user_message();
        }
        GenerateError::NotYetImplemented(message) => {
          error_message = message;
        }
        GenerateError::ProviderRejected(message) => {
          // The provider refused for a reason the user can act on; the
          // message already explains it.
          status = CommandErrorStatus::BadRequest;
          error_message = message;
        }
        _ => {}, // Fall-through
      }

      Err(CommandErrorResponseWrapper {
        status,
        error_message: Some(error_message),
        error_type: Some(error_type),
        error_details: None,
      })
    }
    Ok(event) => {
      let event = GenerationEnqueueSuccessEvent {
        action: event.to_frontend_event_action(),
        service: event.to_frontend_event_service(),
        model: event.model,
      };

      if let Err(err) = event.send(&app) {
        error!("Failed to emit event: {:?}", err);
      }

      CreditsBalanceChangedEvent{}.send_infallible(&app);

      Ok(TauriGenerateAudioResponse {}.into())
    }
  }
}

async fn handle_request(
  request: TauriGenerateAudioRequest,
  app_data_root: &AppDataRoot,
  task_database: &TaskDatabase,
  ledger: &AssetUploadLedger,
) -> Result<TaskEnqueueSuccess, GenerateError> {

  // Generation is credential-driven: the request names a stored credential,
  // and the router dispatches to that credential's service.
  let success_event = handle_credential_router(&request, app_data_root, ledger).await?;

  let result = success_event
    .insert_into_task_database_with_frontend_payload(
      task_database,
      request.frontend_caller,
      request.frontend_subscriber_id.as_deref(),
      request.frontend_subscriber_payload.as_deref(),
    )
    .await;

  if let Err(err) = result {
    error!("Failed to create task in database: {:?}", err);
  }

  // NB: The usage tracker has no audio method yet.

  Ok(success_event)
}
