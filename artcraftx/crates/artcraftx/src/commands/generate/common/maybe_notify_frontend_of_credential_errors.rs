use tauri::AppHandle;

use crate::commands::generate::generate_error::{CredentialProblemReason, GenerateError};
use crate::credentials::login_website::LoginWebsite;
use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::functional_events::show_credential_error_modal_event::ShowCredentialErrorModalEvent;

/// If the error is a credential problem, tell the frontend to show the
/// dismissable credential-error modal. An expired session also names the
/// website and credential to log into again, so the modal can offer a
/// "log in" button that refreshes that exact credential.
pub async fn maybe_notify_frontend_of_credential_errors(
  app: &AppHandle,
  error: &GenerateError,
) {
  let GenerateError::CredentialProblem(reason) = error else {
    return;
  };

  let (maybe_relogin_website, maybe_credential_id) = match reason {
    CredentialProblemReason::SessionExpired { credential_id, service } => (
      LoginWebsite::for_credential_service(*service),
      Some(credential_id.clone()),
    ),
    _ => (None, None),
  };

  ShowCredentialErrorModalEvent {
    message: reason.user_message(),
    maybe_relogin_website,
    maybe_credential_id,
  }.send_infallible(app);
}
