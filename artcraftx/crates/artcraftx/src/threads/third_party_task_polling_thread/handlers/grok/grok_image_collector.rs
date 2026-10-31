use grok_consumer_client::endpoint_bindings::generate_image_websocket::grok_generate_image_websocket::CompletedImage;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Images a Grok prompt has produced so far, keyed by request id.
///
/// Grok's imagine websocket streams a prompt's images one at a time, so the
/// polling thread accumulates them here between iterations and decides when a
/// prompt is done (all expected images, or nothing more for a while).
#[derive(Default)]
pub struct GrokImageCollector {
  pending: HashMap<String, PendingGrokImages>,
}

pub struct PendingGrokImages {
  pub images: Vec<CompletedImage>,
  /// When the polling thread first saw the task pending.
  pub first_seen: Instant,
  /// When the most recent image arrived.
  pub maybe_last_image_at: Option<Instant>,
}

impl GrokImageCollector {
  pub fn new() -> Self {
    Self::default()
  }

  /// Start tracking a request id (no-op if already tracked).
  pub fn track(&mut self, request_id: &str) {
    self.pending.entry(request_id.to_string()).or_insert_with(|| PendingGrokImages {
      images: Vec::new(),
      first_seen: Instant::now(),
      maybe_last_image_at: None,
    });
  }

  /// Record a finished image for its request id (deduped by URL). Returns
  /// whether the id was being tracked.
  pub fn push_image(&mut self, image: CompletedImage) -> bool {
    let Some(pending) = self.pending.get_mut(&image.request_id.0) else {
      return false;
    };
    if !pending.images.iter().any(|existing| existing.url == image.url) {
      pending.images.push(image);
      pending.maybe_last_image_at = Some(Instant::now());
    }
    true
  }

  pub fn get(&self, request_id: &str) -> Option<&PendingGrokImages> {
    self.pending.get(request_id)
  }

  pub fn remove(&mut self, request_id: &str) -> Option<PendingGrokImages> {
    self.pending.remove(request_id)
  }

  /// Drop everything not in `live_request_ids` (tasks that are no longer
  /// pending, e.g. dismissed by the user).
  pub fn retain_only(&mut self, live_request_ids: &[&str]) {
    self.pending.retain(|id, _| live_request_ids.contains(&id.as_str()));
  }
}

impl PendingGrokImages {
  /// Whether the prompt looks finished: every expected image arrived, or at
  /// least one did and nothing more has come for `idle_timeout`.
  pub fn is_complete(&self, expected_images: usize, idle_timeout: Duration) -> bool {
    if self.images.len() >= expected_images {
      return true;
    }
    match self.maybe_last_image_at {
      Some(last) if !self.images.is_empty() => last.elapsed() >= idle_timeout,
      _ => false,
    }
  }

  /// Whether nothing at all arrived within `timeout` of first seeing the task.
  pub fn is_timed_out(&self, timeout: Duration) -> bool {
    self.images.is_empty() && self.first_seen.elapsed() >= timeout
  }
}
