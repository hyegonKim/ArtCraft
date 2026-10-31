use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::bail;
use lru_time_cache::LruCache;

use errors::AnyhowResult;

/// Maintain counters for keys with a TTL
#[derive(Clone)]
pub struct TtlKeyCounter {
  cache: Arc<Mutex<LruCache<String, u64>>>,
}

impl TtlKeyCounter {
  pub fn create_with_duration(time_to_live: Duration) -> Self {
    let cache = LruCache::with_expiry_duration(time_to_live);
    let cache = Arc::new(Mutex::new(cache));
    Self {
      cache,
    }
  }

  pub fn increment_count(&self, key: &str) -> AnyhowResult<u64> {
    match self.cache.lock() {
      Err(e) => bail!("could not unlock mutex to read: {:?}", e),
      Ok(mut cache) => match cache.get_mut(key) {
        None => {
          let _r = cache.insert(key.to_string(), 1);
          Ok(0)
        }
        Some(count) => {
          let return_count = *count;
          *count = count.saturating_add(1);
          Ok(return_count)
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use std::thread;
  use std::time::Duration;

  use crate::ttl_key_counter::TtlKeyCounter;

  #[test]
  fn test_key_first_count() {
    let counter = TtlKeyCounter::create_with_duration(Duration::from_secs(2));

    assert_eq!(0, counter.increment_count("foo").unwrap());
    assert_eq!(0, counter.increment_count("bar").unwrap());
    assert_eq!(0, counter.increment_count("baz").unwrap());
  }

  #[test]
  fn test_increment() {
    let counter = TtlKeyCounter::create_with_duration(Duration::from_secs(2));

    assert_eq!(0, counter.increment_count("foo").unwrap());
    assert_eq!(1, counter.increment_count("foo").unwrap());
    assert_eq!(2, counter.increment_count("foo").unwrap());

    assert_eq!(0, counter.increment_count("bar").unwrap());
    assert_eq!(1, counter.increment_count("bar").unwrap());
    assert_eq!(2, counter.increment_count("bar").unwrap());

    assert_eq!(3, counter.increment_count("foo").unwrap());
    assert_eq!(4, counter.increment_count("foo").unwrap());
    assert_eq!(5, counter.increment_count("foo").unwrap());

    assert_eq!(0, counter.increment_count("baz").unwrap());
    assert_eq!(1, counter.increment_count("baz").unwrap());
    assert_eq!(2, counter.increment_count("baz").unwrap());

    assert_eq!(6, counter.increment_count("foo").unwrap());
    assert_eq!(7, counter.increment_count("foo").unwrap());
    assert_eq!(8, counter.increment_count("foo").unwrap());

    assert_eq!(3, counter.increment_count("bar").unwrap());
    assert_eq!(4, counter.increment_count("bar").unwrap());
    assert_eq!(5, counter.increment_count("bar").unwrap());
  }

  #[test]
  fn test_increment_loop() {
    let counter = TtlKeyCounter::create_with_duration(Duration::from_secs(2));

    for i in 0..100u64 {
      assert_eq!(i, counter.increment_count("foo").unwrap());
      assert_eq!(i, counter.increment_count("bar").unwrap());
    }

    for i in 0..100u64 {
      assert_eq!(i, counter.increment_count("baz").unwrap());
      assert_eq!(i, counter.increment_count("bin").unwrap());

      assert_eq!(i + 100, counter.increment_count("foo").unwrap());
    }
  }

  #[test]
  fn test_expiry_of_key() {
    let counter = TtlKeyCounter::create_with_duration(Duration::from_secs(2));

    assert_eq!(0, counter.increment_count("foo").unwrap());
    assert_eq!(1, counter.increment_count("foo").unwrap());
    assert_eq!(2, counter.increment_count("foo").unwrap());
    assert_eq!(3, counter.increment_count("foo").unwrap());
    assert_eq!(4, counter.increment_count("foo").unwrap());

    // TODO: Use a fake clock!
    thread::sleep(Duration::from_secs(2));

    assert_eq!(0, counter.increment_count("foo").unwrap());
    assert_eq!(1, counter.increment_count("foo").unwrap());
    assert_eq!(2, counter.increment_count("foo").unwrap());
    assert_eq!(3, counter.increment_count("foo").unwrap());
    assert_eq!(4, counter.increment_count("foo").unwrap());
  }

  #[test]
  fn test_update_bumps_ttl() {
    let counter = TtlKeyCounter::create_with_duration(Duration::from_secs(2));

    assert_eq!(0, counter.increment_count("foo").unwrap());
    assert_eq!(1, counter.increment_count("foo").unwrap());
    assert_eq!(2, counter.increment_count("foo").unwrap());

    // TODO: Use a fake clock!
    thread::sleep(Duration::from_secs(1));

    assert_eq!(3, counter.increment_count("foo").unwrap());
    assert_eq!(4, counter.increment_count("foo").unwrap());
    assert_eq!(5, counter.increment_count("foo").unwrap());

    thread::sleep(Duration::from_secs(1));

    assert_eq!(6, counter.increment_count("foo").unwrap());
    assert_eq!(7, counter.increment_count("foo").unwrap());
    assert_eq!(8, counter.increment_count("foo").unwrap());

    thread::sleep(Duration::from_secs(1));

    assert_eq!(9, counter.increment_count("foo").unwrap());
    assert_eq!(10, counter.increment_count("foo").unwrap());
    assert_eq!(11, counter.increment_count("foo").unwrap());

    // Now we expire it...
    thread::sleep(Duration::from_secs(2));

    assert_eq!(0, counter.increment_count("foo").unwrap());
    assert_eq!(1, counter.increment_count("foo").unwrap());
    assert_eq!(2, counter.increment_count("foo").unwrap());
  }
}
