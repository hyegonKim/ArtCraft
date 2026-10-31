//! The wake-up channel between downloaders and the thumbnail worker thread.
//!
//! Downloaders push the paths they just wrote and the worker wakes at once;
//! without a push the worker still scans the databases on its own timer,
//! so a lost signal only delays a thumbnail, never loses it.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use log::{debug, warn};
use tauri::{AppHandle, Manager};
use tokio::sync::Notify;

/// Tauri-managed state. Cheap to clone; all clones share one queue.
#[derive(Clone)]
pub struct ThumbnailWorkQueue {
  inner: Arc<Inner>,
}

struct Inner {
  /// FIFO of paths to thumbnail, deduplicated by `queued`.
  pending: Mutex<PendingPaths>,
  /// Wakes the worker; `notify_one` stores a permit if it isn't waiting.
  notify: Notify,
}

#[derive(Default)]
struct PendingPaths {
  order: VecDeque<PathBuf>,
  queued: HashSet<PathBuf>,
}

impl ThumbnailWorkQueue {
  pub fn new() -> Self {
    Self {
      inner: Arc::new(Inner {
        pending: Mutex::new(PendingPaths::default()),
        notify: Notify::new(),
      }),
    }
  }

  /// Queue freshly downloaded files and wake the worker. Fails open when the
  /// queue isn't managed (headless callers, early startup): the worker's
  /// periodic database scan picks the files up later.
  pub fn enqueue_for_app(app: &AppHandle, file_paths: &[impl AsRef<Path>]) {
    match app.try_state::<ThumbnailWorkQueue>() {
      Some(queue) => queue.enqueue(file_paths),
      None => warn!("[ThumbnailQueue] Not managed yet; {} file(s) will be found by the periodic scan", file_paths.len()),
    }
  }

  /// Queue paths (skipping ones already queued) and wake the worker.
  pub fn enqueue(&self, file_paths: &[impl AsRef<Path>]) {
    if file_paths.is_empty() {
      return;
    }
    let mut added = 0;
    {
      let mut pending = self.inner.pending.lock().expect("thumbnail queue lock poisoned");
      for file_path in file_paths {
        let file_path = file_path.as_ref().to_path_buf();
        if pending.queued.insert(file_path.clone()) {
          pending.order.push_back(file_path);
          added += 1;
        }
      }
    }
    debug!("[ThumbnailQueue] Queued {} new path(s)", added);
    self.inner.notify.notify_one();
  }

  /// Wake the worker without queueing anything (e.g. to force a scan).
  pub fn wake(&self) {
    self.inner.notify.notify_one();
  }

  /// Take everything queued so far, in FIFO order.
  pub fn drain(&self) -> Vec<PathBuf> {
    let mut pending = self.inner.pending.lock().expect("thumbnail queue lock poisoned");
    pending.queued.clear();
    pending.order.drain(..).collect()
  }

  /// Resolves on the next wake-up (or immediately if one is already stored).
  pub async fn notified(&self) {
    self.inner.notify.notified().await
  }
}

impl Default for ThumbnailWorkQueue {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn drain_returns_fifo_without_duplicates() {
    let queue = ThumbnailWorkQueue::new();
    queue.enqueue(&["/a.mp4", "/b.mp4"]);
    queue.enqueue(&["/a.mp4", "/c.mp4"]);
    let drained = queue.drain();
    assert_eq!(drained, vec![PathBuf::from("/a.mp4"), PathBuf::from("/b.mp4"), PathBuf::from("/c.mp4")]);
    assert!(queue.drain().is_empty());
    // Once drained a path can be queued again (its content may have changed).
    queue.enqueue(&["/a.mp4"]);
    assert_eq!(queue.drain().len(), 1);
  }

  #[tokio::test]
  async fn enqueue_wakes_a_waiter_even_before_it_waits() {
    let queue = ThumbnailWorkQueue::new();
    queue.enqueue(&["/a.mp4"]);
    // The permit was stored, so this completes immediately.
    tokio::time::timeout(std::time::Duration::from_millis(100), queue.notified()).await.unwrap();
  }
}
