use crate::events::basic_sendable_event_trait::{BasicEventStatus, BasicSendableEvent};
use artcraft_client::enums::tauri::ux::tauri_event_name::TauriEventName;
use serde_derive::Serialize;

/// The thumbnail worker finished a local file. Paths are absolute; render
/// them through Tauri's asset protocol (`convertFileSrc`).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct LocalThumbnailReadyEvent {
  /// The source file the thumbnail is for (keys the frontend's cache).
  pub file_path: String,
  /// Static jpg in the thumbnail cache.
  pub thumbnail_path: String,
  /// ~1s animated webp, videos only.
  pub maybe_animated_preview_path: Option<String>,
}

impl BasicSendableEvent for LocalThumbnailReadyEvent {
  const FRONTEND_EVENT_NAME: TauriEventName = TauriEventName::LocalThumbnailReadyEvent;
  const EVENT_STATUS: BasicEventStatus = BasicEventStatus::Success;
}
