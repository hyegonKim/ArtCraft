use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::bail;
use lru_time_cache::LruCache;

use errors::AnyhowResult;

/// Essentially a wrapper around LruCache.
#[derive(Clone)]
pub struct MultiItemTtlCache<K: Ord + Clone, V: Clone + ?Sized> {
  cache: Arc<Mutex<LruCache<K, V>>>,
}

impl <K: Ord + Clone, V: Clone + ?Sized> MultiItemTtlCache<K, V> {
  pub fn create_with_duration(time_to_live: Duration) -> Self {
    let cache = LruCache::with_expiry_duration(time_to_live);
    let cache = Arc::new(Mutex::new(cache));
    Self {
      cache,
    }
  }

  pub fn copy_without_bump_if_unexpired(&self, key: K) -> AnyhowResult<Option<V>> {
    let maybe_copy = match self.cache.lock() {
      Err(e) => bail!("could not unlock mutex to read: {:?}", e),
      Ok(cache) => {
        cache.peek(&key).map(|inner| inner.clone())
      },
    };
    Ok(maybe_copy)
  }

  pub fn store(&self, key: K, value: V) -> AnyhowResult<()> {
    match self.cache.lock() {
      Err(e) => bail!("could not unlock mutex to write: {:?}", e),
      Ok(mut cache) => {
        cache.insert(key, value);
      },
    };
    Ok(())
  }

  pub fn store_copy(&self, key: &K, value: &V) -> AnyhowResult<()> {
    match self.cache.lock() {
      Err(e) => bail!("could not unlock mutex to write: {:?}", e),
      Ok(mut cache) => {
        cache.insert(key.clone(), value.clone());
      },
    };
    Ok(())
  }
}
