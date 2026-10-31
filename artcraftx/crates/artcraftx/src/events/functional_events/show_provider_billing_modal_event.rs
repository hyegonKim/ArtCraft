use crate::events::basic_sendable_event_trait::{BasicEventStatus, BasicSendableEvent};
use core_types::enums::generation_source::GenerationSource;
use artcraft_client::enums::tauri::ux::tauri_event_name::TauriEventName;
use serde_derive::Serialize;
use tauri::AppHandle;

/// Send a signal to the frontend to show a modal that suggests fixing or setting up 
/// service billing requirements.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ShowProviderBillingModalEvent {
  pub provider: GenerationSource,
}

impl ShowProviderBillingModalEvent {
  pub fn send_for_provider(provider: GenerationSource, app: &AppHandle) {
    let event = Self { provider };
    event.send_infallible(&app);
  }
}

impl BasicSendableEvent for ShowProviderBillingModalEvent {
  const FRONTEND_EVENT_NAME: TauriEventName = TauriEventName::ShowProviderBillingModalEvent;
  const EVENT_STATUS: BasicEventStatus = BasicEventStatus::Failure;
}
