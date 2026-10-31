use midjourney_client::client::websocket::midjourney_websocket::MidjourneyWebSocket;
use midjourney_client::credentials::midjourney_user_id::MidjourneyUserId;
use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

/// In-memory, process-lifetime Midjourney session state.
///
/// Distinct from the on-disk cookie credential (which lives in the TOML
/// credential store): this holds the *live* session bits that are cheap to
/// re-derive and should not be persisted — the resolved `user_id`, the
/// short-lived `websocket_token`, and the currently-open websocket handle.
///
/// All fields start `None`. On the first enqueue we resolve the identity and
/// open a websocket; if the websocket dies we drop the handle so the next
/// enqueue re-opens one. The completion path prefers a live websocket and only
/// falls back to the HTTP status/download endpoints when it has terminated.
///
/// Cheaply clonable (`Arc`-backed); every clone shares the same state.
#[derive(Clone, Default)]
pub struct MidjourneyLiveSession {
  inner: Arc<RwLock<MidjourneyLiveSessionInner>>,
}

#[derive(Default)]
struct MidjourneyLiveSessionInner {
  user_id: Option<MidjourneyUserId>,
  websocket_token: Option<String>,
  websocket: Option<MidjourneyWebSocket>,

  /// Prompt text captured at enqueue, keyed by Midjourney job id. The
  /// websocket completion path uses it to attribute the created Storyteller
  /// prompt (the `Completed` frame does not carry the prompt). Consumed on
  /// finalize.
  pending_prompts: HashMap<String, String>,
}

impl MidjourneyLiveSession {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn user_id(&self) -> Option<MidjourneyUserId> {
    self.read(|inner| inner.user_id.clone())
  }

  pub fn websocket_token(&self) -> Option<String> {
    self.read(|inner| inner.websocket_token.clone())
  }

  /// Record the identity resolved from `get_user_info` (index-page scrape).
  pub fn set_identity(&self, user_id: MidjourneyUserId, websocket_token: Option<String>) {
    self.write(|inner| {
      inner.user_id = Some(user_id);
      if websocket_token.is_some() {
        inner.websocket_token = websocket_token;
      }
    });
  }

  /// The live websocket handle if one is currently open AND still connected.
  /// A terminated socket is dropped as a side effect so the next enqueue
  /// re-opens one.
  pub fn connected_websocket(&self) -> Option<MidjourneyWebSocket> {
    // Fast path: read lock to check.
    let maybe_connected = self.read(|inner| {
      inner.websocket.as_ref().map(|ws| (ws.clone(), ws.is_connected()))
    });

    match maybe_connected {
      Some((ws, true)) => Some(ws),
      Some((_, false)) => {
        self.clear_websocket();
        None
      }
      None => None,
    }
  }

  pub fn set_websocket(&self, websocket: MidjourneyWebSocket) {
    self.write(|inner| inner.websocket = Some(websocket));
  }

  /// Remember the prompt for a just-enqueued job so the completion path can
  /// attribute it even when the source (websocket `Completed`) omits it.
  pub fn record_pending_prompt(&self, job_id: String, prompt: String) {
    self.write(|inner| {
      inner.pending_prompts.insert(job_id, prompt);
    });
  }

  /// Consume the stashed prompt for a job (returns `None` if unknown).
  pub fn take_pending_prompt(&self, job_id: &str) -> Option<String> {
    self.write(|inner| inner.pending_prompts.remove(job_id))
  }

  pub fn clear_websocket(&self) {
    self.write(|inner| inner.websocket = None);
  }

  /// Clears everything (e.g. on logout / credential change).
  pub fn clear(&self) {
    self.write(|inner| {
      inner.user_id = None;
      inner.websocket_token = None;
      inner.websocket = None;
      inner.pending_prompts.clear();
    });
  }

  fn read<R>(&self, reader: impl FnOnce(&MidjourneyLiveSessionInner) -> R) -> R {
    let guard = self.inner.read().unwrap_or_else(PoisonError::into_inner);
    reader(&guard)
  }

  fn write<R>(&self, writer: impl FnOnce(&mut MidjourneyLiveSessionInner) -> R) -> R {
    let mut guard = self.inner.write().unwrap_or_else(PoisonError::into_inner);
    writer(&mut guard)
  }
}
