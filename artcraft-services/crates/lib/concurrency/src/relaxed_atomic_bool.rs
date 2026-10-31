use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Relaxed AtomicBool.
/// These are not meant to be used for coordinated synchronization, just easy cross-thread sharing
/// with interior mutability.
#[derive(Clone)]
pub struct RelaxedAtomicBool {
  value: Arc<AtomicBool>
}

impl RelaxedAtomicBool {
  pub fn new(value: bool) -> Self {
    Self {
      value: Arc::new(AtomicBool::new(value)),
    }
  }

  pub fn get(&self) -> bool {
    self.value.load(Ordering::Relaxed)
  }

  pub fn set(&self, value: bool) {
    self.value.store(value, Ordering::Relaxed)
  }
}

#[cfg(test)]
mod tests {
  use std::thread;
  use std::time::{Duration, Instant};

  use crate::relaxed_atomic_bool::RelaxedAtomicBool;

  #[test]
  fn test_get_and_set() {
    let b = RelaxedAtomicBool::new(false);
    assert_eq!(false, b.get());

    b.set(true);
    assert_eq!(true, b.get());

    b.set(false);
    assert_eq!(false, b.get());
  }

  #[test]
  fn test_share_threads_read() {
    // NB: This is mainly to test that the code compiles
    let a = RelaxedAtomicBool::new(true);
    let b = a.clone();
    let c = a.clone();
    let d = a.clone();

    thread::spawn(move || assert_eq!(true, a.get()));
    thread::spawn(move || assert_eq!(true, b.get()));
    thread::spawn(move || assert_eq!(true, c.get()));

    assert_eq!(true, d.get());
  }

  #[test]
  fn test_share_threads_write() {

    // NB: This is mainly to test that the code compiles
    let keep_looping = RelaxedAtomicBool::new(true);
    let another = keep_looping.clone();

    assert_eq!(true, keep_looping.get());

    thread::spawn(move || another.set(false));

    let start = Instant::now();
    let limit = Duration::from_secs(5);

    while keep_looping.get() {
      if start.elapsed().gt(&limit) {
        panic!("Thread did not set the correct value in 5 secs!");
      }
    }

    assert_eq!(false, keep_looping.get());
  }
}
