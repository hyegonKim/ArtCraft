use crate::events::basic_sendable_event_trait::{BasicEventStatus, BasicSendableEvent};
use artcraft_client::enums::tauri::ux::tauri_event_name::TauriEventName;
use serde_derive::Serialize;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use url::Url;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ObjectGenerationCompleteEvent {
  pub generated_objects: Vec<GeneratedObject>,
  pub maybe_frontend_subscriber_id: Option<String>,
  pub maybe_frontend_subscriber_payload: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GeneratedObject {
  pub media_token: MediaFileToken,
  pub cdn_url: Url,
  pub maybe_thumbnail_template: Option<String>,
}

impl BasicSendableEvent for ObjectGenerationCompleteEvent {
  const FRONTEND_EVENT_NAME: TauriEventName = TauriEventName::ObjectGenerationCompleteEvent;
  const EVENT_STATUS: BasicEventStatus = BasicEventStatus::Success;
}
