use crate::events::basic_sendable_event_trait::{BasicEventStatus, BasicSendableEvent};
use artcraft_client::enums::tauri::ux::tauri_event_name::TauriEventName;
use serde_derive::Serialize;
use tauri::AppHandle;
use uuid_utils::uuid::generate_random_uuid;

/// A long-running backend step the user has to wait on, e.g. Higgsfield's
/// IP/likeness check on uploaded media (routinely 15-30s). Send `started`
/// before the step and `finished` after it; the frontend keeps a progress
/// toast up in between. Prefer the [`ProgressNotice`] guard over sending
/// these by hand so the `finished` half can't be forgotten on error paths.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ProgressNoticeEvent {
  /// Ties `finished` to its `started`; unique per step so concurrent steps
  /// get their own toasts.
  pub notice_id: String,
  pub state: ProgressNoticeState,
  /// What the user is waiting on. Only set on `started`.
  pub maybe_message: Option<String>,
}

#[derive(Copy, Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressNoticeState {
  Started,
  Finished,
}

impl BasicSendableEvent for ProgressNoticeEvent {
  const FRONTEND_EVENT_NAME: TauriEventName = TauriEventName::ProgressNoticeEvent;
  const EVENT_STATUS: BasicEventStatus = BasicEventStatus::Success;
}

/// RAII handle for a [`ProgressNoticeEvent`] pair: sends `started` on
/// creation and `finished` when dropped, so holding it across an `.await`
/// scopes the toast to that work — including early returns and `?` errors.
///
/// ```ignore
/// let _notice = ProgressNotice::start(app, "Higgsfield is checking your media");
/// upload_and_enqueue().await?; // toast comes down however this ends
/// ```
#[must_use = "dropping the notice immediately ends it; bind it to a variable"]
pub struct ProgressNotice {
  app: AppHandle,
  notice_id: String,
}

impl ProgressNotice {
  pub fn start(app: &AppHandle, message: impl Into<String>) -> Self {
    let notice_id = generate_random_uuid();
    ProgressNoticeEvent {
      notice_id: notice_id.clone(),
      state: ProgressNoticeState::Started,
      maybe_message: Some(message.into()),
    }.send_infallible(app);
    Self { app: app.clone(), notice_id }
  }
}

impl Drop for ProgressNotice {
  fn drop(&mut self) {
    ProgressNoticeEvent {
      notice_id: self.notice_id.clone(),
      state: ProgressNoticeState::Finished,
      maybe_message: None,
    }.send_infallible(&self.app);
  }
}
