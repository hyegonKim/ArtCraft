use crate::events::basic_sendable_event_trait::{BasicEventStatus, BasicSendableEvent};
use crate::events::generation_events::common::{GenerationAction, GenerationModel, GenerationServiceProvider};
use artcraft_client::enums::tauri::ux::tauri_event_name::TauriEventName;
use serde_derive::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GenerationCompleteEvent {
  pub action: Option<GenerationAction>,
  pub service: GenerationServiceProvider,
  pub model: Option<GenerationModel>,
}

impl BasicSendableEvent for GenerationCompleteEvent {
  const FRONTEND_EVENT_NAME: TauriEventName = TauriEventName::GenerationCompleteEvent;
  const EVENT_STATUS: BasicEventStatus = BasicEventStatus::Success;
}
