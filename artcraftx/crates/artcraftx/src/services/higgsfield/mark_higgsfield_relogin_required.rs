use log::{error, info, warn};
use tauri::AppHandle;

use crate::commands::generate::generate_error::CredentialProblemReason;
use crate::credentials::login_website::LoginWebsite;
use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::functional_events::refresh_account_state_event::RefreshAccountStateEvent;
use crate::events::functional_events::show_credential_error_modal_event::ShowCredentialErrorModalEvent;
use crate::state::data_dir::app_data_root::AppDataRoot;
use core_types::enums::generation_source::GenerationSource;

/// Background paths (task polling) found that Higgsfield rejected a stored
/// session. Mark the credential file so every caller skips it until the
/// user logs in again, and — the first time only — show the "log in again"
/// modal and refresh the account state. Silent if already marked.
pub fn mark_higgsfield_relogin_required(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  credential_id: &str,
) {
  let maybe_credential = match app_data_root.credentials_dir().find_credential_by_id(credential_id) {
    Ok(maybe_credential) => maybe_credential,
    Err(err) => {
      error!("Could not load credential {} to mark it as needing a re-login: {}", credential_id, err);
      return;
    }
  };
  let Some(mut credential) = maybe_credential else {
    warn!("Credential {} is gone; nothing to mark as needing a re-login", credential_id);
    return;
  };

  match credential.mark_relogin_required() {
    Ok(true) => info!("Marked Higgsfield credential {} as needing a re-login", credential_id),
    Ok(false) => return,
    Err(err) => {
      error!("Could not mark credential {} as needing a re-login: {}", credential_id, err);
      return;
    }
  }

  let reason = CredentialProblemReason::SessionExpired {
    credential_id: credential.id.to_string(),
    service: credential.service,
  };
  ShowCredentialErrorModalEvent {
    message: reason.user_message(),
    maybe_relogin_website: LoginWebsite::for_credential_service(credential.service),
    maybe_credential_id: Some(credential.id.to_string()),
  }.send_infallible(app_handle);
  RefreshAccountStateEvent { provider: Some(GenerationSource::Higgsfield) }.send_infallible(app_handle);
}
