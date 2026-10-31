use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use errors::{anyhow, AnyhowResult};

/// Sometimes I need to kill a thread from somewhere else. This is a mechanism
/// that enables it. This utility can be combined with logic that will kill a
/// thread's JoinHandle.
/// NB: This was designed with interior mutability and clone-ability in mind.

#[derive(Clone)]
pub struct AsyncThreadKillSignal {
  internal: Arc<RwLock<AsyncThreadKillSignalInternal>>,
}

#[derive(Clone)]
struct AsyncThreadKillSignalInternal {
  ttl: Option<Duration>,
  last_refreshed: Instant,
  manually_killed: bool,
}

impl AsyncThreadKillSignal {

  pub fn new_with_ttl(ttl: Duration) -> Self {
    Self {
      internal: Arc::new(RwLock::new(AsyncThreadKillSignalInternal {
        last_refreshed: Instant::now(),
        ttl: Some(ttl),
        manually_killed: false,
      }))
    }
  }

  /// Whether we should keep the thread alive.
  pub fn is_alive(&self) -> AnyhowResult<bool> {
    match self.internal.read() {
      Err(e) => {
        Err(anyhow!("lock issue: {:?}", e))
      },
      Ok(internal) => {
        if let Some(ttl) = &internal.ttl {
          let now = Instant::now();
          if now.duration_since(internal.last_refreshed).gt(ttl) {
            return Ok(false);
          }
        }
        Ok(!internal.manually_killed)
      },
    }
  }

  /// If the thread has a TTL, refresh it.
  pub fn bump_ttl(&self) -> AnyhowResult<()> {
    match self.internal.write() {
      Err(e) => {
        Err(anyhow!("lock issue: {:?}", e))
      },
      Ok(mut internal) => {
        internal.last_refreshed = Instant::now();
        Ok(())
      },
    }
  }

  /// Tell the thread it should die.
  pub fn mark_thread_for_kill(&self) -> AnyhowResult<()> {
    match self.internal.write() {
      Err(e) => {
        Err(anyhow!("lock issue: {:?}", e))
      },
      Ok(mut internal) => {
        internal.manually_killed = true;
        Ok(())
      },
    }
  }
}
