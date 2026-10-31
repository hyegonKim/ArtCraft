use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::bail;
use lru_time_cache::LruCache;

use errors::AnyhowResult;

/// NB: There's only ONE ITEM of ONE TYPE in the cache. We can use a single key.
const CACHE_KEY : &str = "ITEM";

/// Store a single payload in the cache
/// There's only ONE ITEM of ONE TYPE in the cache.
/// This is essentially a singleton cache with expiry.
#[derive(Clone)]
pub struct SingleItemTtlCache<T: Clone + ?Sized> {
  cache: Arc<Mutex<LruCache<String, T>>>,
}

impl <T: Clone + ?Sized> SingleItemTtlCache<T> {
  pub fn create_with_duration(time_to_live: Duration) -> Self {
    let cache = LruCache::with_expiry_duration(time_to_live);
    let cache = Arc::new(Mutex::new(cache));
    Self {
      cache,
    }
  }

  pub fn grab_copy_without_bump_if_unexpired(&self) -> AnyhowResult<Option<T>> {
    let maybe_copy = match self.cache.lock() {
      Err(e) => bail!("could not unlock mutex to read: {:?}", e),
      Ok(cache) => {
        cache.peek(CACHE_KEY).map(|inner| inner.clone())
      },
    };
    Ok(maybe_copy)
  }

  /// NB: This was designed to prevent cold cache / DDoS
  pub fn grab_even_expired_and_bump(&self) -> AnyhowResult<Option<T>> {
    match self.cache.lock() {
      Err(e) => bail!("could not unlock mutex to read: {:?}", e),
      Ok(mut cache) => {
        let (maybe_unexpired_entry, maybe_expired_entries) = cache.notify_get(CACHE_KEY);

        if let Some(unexpired_entry) = maybe_unexpired_entry {
          return Ok(Some(unexpired_entry.clone()));
        }

        for (key, expired_entry) in maybe_expired_entries.iter() {
          // Put the entry right back into the cache with a fresh expiry.
          if key == CACHE_KEY {
            cache.insert(CACHE_KEY.into(), expired_entry.clone());
            return Ok(Some(expired_entry.clone()));
          }
        }

        Ok(None)
      },
    }
  }

  pub fn store_copy(&self, item: &T) -> AnyhowResult<()> {
    match self.cache.lock() {
      Err(e) => bail!("could not unlock mutex to write: {:?}", e),
      Ok(mut cache) => {
        cache.insert(CACHE_KEY.into(), item.clone());
      },
    };
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use std::thread;
  use std::time::Duration;

  use crate::single_item_ttl_cache::SingleItemTtlCache;

  #[test]
  fn test_grab_copy_without_bump_if_unexpired() {
    let cache = SingleItemTtlCache::create_with_duration(Duration::from_secs(2));

    let original = "foo".to_string();
    cache.store_copy(&original).unwrap();

    assert_eq!(cache.grab_copy_without_bump_if_unexpired().unwrap(), Some(original.clone()));
    assert_eq!(cache.grab_copy_without_bump_if_unexpired().unwrap(), Some(original.clone()));
    assert_eq!(cache.grab_copy_without_bump_if_unexpired().unwrap(), Some(original.clone()));

    // TODO: Use a fake clock!
    thread::sleep(Duration::from_secs(2));

    assert_eq!(cache.grab_copy_without_bump_if_unexpired().unwrap(), None);
    assert_eq!(cache.grab_copy_without_bump_if_unexpired().unwrap(), None);
    assert_eq!(cache.grab_copy_without_bump_if_unexpired().unwrap(), None);
  }

  #[test]
  fn test_grab_even_expired_and_bump() {
    let cache = SingleItemTtlCache::create_with_duration(Duration::from_secs(2));

    let original = "foo".to_string();
    cache.store_copy(&original).unwrap();

    assert_eq!(cache.grab_even_expired_and_bump().unwrap(), Some(original.clone()));
    assert_eq!(cache.grab_even_expired_and_bump().unwrap(), Some(original.clone()));
    assert_eq!(cache.grab_even_expired_and_bump().unwrap(), Some(original.clone()));

    // TODO: Use a fake clock!
    thread::sleep(Duration::from_secs(2));

    // It expired.
    assert_eq!(cache.grab_copy_without_bump_if_unexpired().unwrap(), None);

    // This method can read it.
    assert_eq!(cache.grab_even_expired_and_bump().unwrap(), Some(original.clone()));

    // And now we can read it again.
    assert_eq!(cache.grab_copy_without_bump_if_unexpired().unwrap(), Some(original.clone()));
    assert_eq!(cache.grab_copy_without_bump_if_unexpired().unwrap(), Some(original.clone()));
    assert_eq!(cache.grab_copy_without_bump_if_unexpired().unwrap(), Some(original.clone()));

    // TODO: Use a fake clock!
    thread::sleep(Duration::from_secs(2));

    // It expired again.
    assert_eq!(cache.grab_copy_without_bump_if_unexpired().unwrap(), None);

    // This method can read it (again).
    assert_eq!(cache.grab_even_expired_and_bump().unwrap(), Some(original.clone()));

    // And now we can read it again (again).
    assert_eq!(cache.grab_copy_without_bump_if_unexpired().unwrap(), Some(original.clone()));
  }
}
