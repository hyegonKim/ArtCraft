use crate::events::basic_sendable_event_trait::{BasicEventStatus, BasicSendableEvent};
use artcraft_client::enums::tauri::ux::tauri_event_name::TauriEventName;
use serde_derive::Serialize;

/// The app preferences were saved (by any command). Carries no payload; the
/// frontend should re-read them via `get_app_preferences_command`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AppPreferencesChangedEvent {}

impl BasicSendableEvent for AppPreferencesChangedEvent {
  const FRONTEND_EVENT_NAME: TauriEventName = TauriEventName::AppPreferencesChangedEvent;
  const EVENT_STATUS: BasicEventStatus = BasicEventStatus::Success;
}
