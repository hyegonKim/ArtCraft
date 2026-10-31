use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::promptbox::promptbox_state::PromptboxState;
use errors::AnyhowResult;
use log::warn;
use memory_store::clone_cell::CloneCell;
use std::sync::{Arc, Mutex, MutexGuard};

/// Shared, process-wide prompt box state: an in-memory copy that readers
/// clone, and a single write path that persists to disk.
#[derive(Clone)]
pub struct PromptboxStateManager {
  state: CloneCell<PromptboxState>,
  data_root: AppDataRoot,
  /// Serializes `update` so concurrent commands don't race each other's
  /// read-modify-write or collide on the same temp file when saving.
  /// The guarded section is synchronous and tiny (clone, serialize a few KB,
  /// write, rename), so callers queue for microseconds, never the UI.
  write_lock: Arc<Mutex<()>>,
}

impl PromptboxStateManager {
  pub fn load_or_default(data_root: &AppDataRoot) -> Self {
    Self {
      state: CloneCell::with_owned(PromptboxState::load_or_default(data_root)),
      data_root: data_root.clone(),
      write_lock: Arc::new(Mutex::new(())),
    }
  }

  pub fn get(&self) -> AnyhowResult<PromptboxState> {
    self.state.get_clone()
  }

  /// Apply a change, persist it, and return the updated snapshot. Nothing
  /// changes in memory if the write fails.
  pub fn update(&self, change: impl FnOnce(&mut PromptboxState)) -> AnyhowResult<PromptboxState> {
    let _guard = self.acquire_write_lock();
    let mut state = self.get()?;
    change(&mut state);
    state.save(&self.data_root)?;
    self.state.set_clone(&state)?;
    Ok(state)
  }

  /// The lock guards nothing but ordering (the real state lives in the cell
  /// and on disk, and saves are atomic), so a poisoned lock is safe to reuse.
  /// Failing here forever would just break a low-stakes preference feature.
  fn acquire_write_lock(&self) -> MutexGuard<'_, ()> {
    self.write_lock.lock().unwrap_or_else(|poisoned| {
      warn!("Prompt box write lock was poisoned by an earlier panic; continuing");
      poisoned.into_inner()
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::thread;

  const THREADS: usize = 8;
  const UPDATES_PER_THREAD: usize = 25;

  /// Regression: concurrent updates used to race on the shared temp file
  /// (`rename` -> ENOENT) and clobber each other's changes.
  #[test]
  fn concurrent_updates_all_succeed_and_none_are_lost() {
    let dir = tempfile::tempdir().unwrap();
    let data_root = AppDataRoot::create_existing(dir.path()).unwrap();
    let manager = PromptboxStateManager::load_or_default(&data_root);

    let handles: Vec<_> = (0..THREADS)
        .map(|thread_index| {
          let manager = manager.clone();
          thread::spawn(move || {
            for i in 0..UPDATES_PER_THREAD {
              manager
                  .update(|state| {
                    state.last_account_by_model.insert(format!("model_{thread_index}_{i}"), "credential".to_string());
                  })
                  .expect("update should never fail under contention");
            }
          })
        })
        .collect();
    for handle in handles {
      handle.join().unwrap();
    }

    let expected = THREADS * UPDATES_PER_THREAD;
    assert_eq!(manager.get().unwrap().last_account_by_model.len(), expected, "in memory");
    let on_disk = PromptboxState::load_or_default(&data_root);
    assert_eq!(on_disk.last_account_by_model.len(), expected, "on disk");
  }
}
