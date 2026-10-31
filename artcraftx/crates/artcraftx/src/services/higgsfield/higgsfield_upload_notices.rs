use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use router::api::asset_upload_observer::AssetUploadObserver;
use tauri::AppHandle;

use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::notice_events::flash_notice_event::FlashNoticeEvent;
use crate::events::notice_events::progress_notice_event::ProgressNotice;

const IP_CHECK_MESSAGE: &str = "Higgsfield is checking your media for Intellectual Property and Likeness";
const UPLOADING_MESSAGE: &str = "Uploading your media to Higgsfield";
const REUSED_MESSAGE: &str = "Reusing media Higgsfield already has (no upload or new IP check needed)";

/// Turns the router's per-file upload decisions into frontend notices for
/// one Higgsfield request:
///
/// - the first real upload puts up a progress toast (the IP/likeness check
///   routinely takes 15-30s), which stays up until this value is dropped —
///   so hold it across the enqueue, including retries;
/// - a reuse flashes a plain, progress-free notice once, since nothing is
///   being waited on.
///
/// Without an app handle (tests, headless callers) it observes silently.
pub struct HiggsfieldUploadNotices {
  maybe_app: Option<AppHandle>,
  progress: Mutex<Option<ProgressNotice>>,
  reuse_announced: AtomicBool,
}

impl HiggsfieldUploadNotices {
  pub fn new(maybe_app: Option<&AppHandle>) -> Self {
    Self {
      maybe_app: maybe_app.cloned(),
      progress: Mutex::new(None),
      reuse_announced: AtomicBool::new(false),
    }
  }
}

impl AssetUploadObserver for HiggsfieldUploadNotices {
  fn on_reused(&self, _kind: &str, _description: &str) {
    let Some(app) = &self.maybe_app else { return };
    if self.reuse_announced.swap(true, Ordering::SeqCst) {
      return;
    }
    FlashNoticeEvent::new(REUSED_MESSAGE).send_infallible(app);
  }

  fn on_uploading(&self, _kind: &str, _byte_count: u64, ip_check: bool) {
    let Some(app) = &self.maybe_app else { return };
    let Ok(mut progress) = self.progress.lock() else { return };
    if progress.is_some() {
      return; // One toast covers every upload of the request.
    }
    let message = if ip_check { IP_CHECK_MESSAGE } else { UPLOADING_MESSAGE };
    *progress = Some(ProgressNotice::start(app, message));
  }
}
