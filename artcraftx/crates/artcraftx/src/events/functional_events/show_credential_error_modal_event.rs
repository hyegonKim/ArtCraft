use crate::events::basic_sendable_event_trait::{BasicEventStatus, BasicSendableEvent};
use crate::credentials::login_website::LoginWebsite;
use artcraft_client::enums::tauri::ux::tauri_event_name::TauriEventName;
use serde_derive::Serialize;

/// Tell the frontend to show a dismissable modal explaining a credential
/// problem: the request named no credential, the credential id didn't match
/// anything on disk, or the credential can't be used for the request.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ShowCredentialErrorModalEvent {
  pub message: String,
  /// When the fix is to log into a website again, which one — the frontend
  /// shows a button that opens that login window.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_relogin_website: Option<LoginWebsite>,
  /// The credential the re-login should refresh in place (rather than
  /// adding a second account for the same service).
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_credential_id: Option<String>,
}

impl BasicSendableEvent for ShowCredentialErrorModalEvent {
  const FRONTEND_EVENT_NAME: TauriEventName = TauriEventName::ShowCredentialErrorModalEvent;
  const EVENT_STATUS: BasicEventStatus = BasicEventStatus::Failure;
}
