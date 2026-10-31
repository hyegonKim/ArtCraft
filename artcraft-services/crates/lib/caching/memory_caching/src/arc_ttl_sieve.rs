use std::hash::Hash;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail};
use sieve_cache::SieveCache;

use errors::AnyhowResult;

#[derive(Clone)]
struct SieveItemWithTtl<V: Clone + ?Sized> {
  value: V,
  stored_at: Instant,
}

impl <V: Clone + ?Sized> SieveItemWithTtl<V> {
  fn new(value: V) -> Self {
    Self {
      value,
      stored_at: Instant::now(),
    }
  }
}

#[derive(Clone)]
pub struct ArcTtlSieve<K: Eq + Hash + Clone, V: Clone + ?Sized> {
  cache: Arc<Mutex<SieveCache<K, SieveItemWithTtl<V>>>>,
  /// Purge items from the sieve if they're requested after this duration.
  /// Do not bump the expiry on successful lookups against the sieve.
  ttl_duration: Duration,
}

impl <K: Eq + Hash + Clone, V: Clone + ?Sized> ArcTtlSieve<K, V> {

  pub fn with_capacity_and_ttl_duration(capacity: usize, ttl_duration: Duration) -> AnyhowResult<Self> {
    let cache = SieveCache::new(capacity)
        .map_err(|err| anyhow!("could not create cache: {:?}", err))?;
    Ok(Self {
      cache: Arc::new(Mutex::new(cache)),
      ttl_duration,
    })
  }

  pub fn store(&self, key: K, value: V) -> AnyhowResult<()> {
    match self.cache.lock() {
      Err(e) => bail!("could not unlock mutex to write: {:?}", e),
      Ok(mut cache) => {
        let item = SieveItemWithTtl::new(value);
        cache.insert(key, item);
      },
    };
    Ok(())
  }

  pub fn store_copy(&self, key: &K, value: &V) -> AnyhowResult<()> {
    match self.cache.lock() {
      Err(e) => bail!("could not unlock mutex to write: {:?}", e),
      Ok(mut cache) => {
        let item = SieveItemWithTtl::new(value.clone());
        cache.insert(key.clone(), item);
      },
    };
    Ok(())
  }

  pub fn get_copy(&self, key: &K) -> AnyhowResult<Option<V>> {
    match self.cache.lock() {
      Err(e) => bail!("could not unlock mutex to read: {:?}", e),
      Ok(mut cache) => {
        let maybe_item = cache.get(key);
        if let Some(item) = maybe_item {
          if item.stored_at.elapsed() < self.ttl_duration {
            return Ok(Some(item.value.clone()));
          } else {
            // NB: The item has elapsed its TTL, so we'll drop it.
            cache.remove(key);
          }
        }
        return Ok(None);
      },
    }
  }
}

#[cfg(test)]
mod tests {
  use std::thread;
  use std::time::Duration;
  use crate::arc_ttl_sieve::ArcTtlSieve;

  #[test]
  fn test_get_copy() {
    let sieve : ArcTtlSieve<String, String> =
        ArcTtlSieve::with_capacity_and_ttl_duration(1, Duration::from_secs(100))
            .unwrap();

    let key = "key".to_string();
    let value = "value".to_string();

    sieve.store_copy(&key, &value).unwrap();
    let maybe_value = sieve.get_copy(&key).unwrap();
    assert_eq!(maybe_value, Some("value".to_string()));
  }

  #[test]
  fn test_simple_expire() {
    let sieve : ArcTtlSieve<String, String> =
        ArcTtlSieve::with_capacity_and_ttl_duration(1, Duration::ZERO)
            .unwrap();

    let key = "key".to_string();
    let value = "value".to_string();

    sieve.store_copy(&key, &value).unwrap();
    let maybe_value = sieve.get_copy(&key).unwrap();

    assert_eq!(maybe_value, None);
  }

  #[test]
  fn test_simple_get_then_expire() {
    let sieve : ArcTtlSieve<String, String> =
        ArcTtlSieve::with_capacity_and_ttl_duration(1, Duration::from_secs(2))
            .unwrap();

    let key = "key".to_string();
    let value = "value".to_string();

    sieve.store_copy(&key, &value).unwrap();

    let mut count = 0;

    for _ in 0 .. 10_000_000 {
      let maybe_value = sieve.get_copy(&key).unwrap();

      if maybe_value.is_none() {
        break;
      } else {
        assert_eq!(maybe_value, Some("value".to_string()));
        count += 1;
      }
    }

    // NB: We should be able to retrieve the item roughly over a million times in two seconds,
    // but in case there's an interrupt we'll only check that we got it a few times.
    assert!(count > 3);

    let mut maybe_value = sieve.get_copy(&key).unwrap();

    if maybe_value.is_some() {
      // The TTL should have expired by now, but just in case...
      thread::sleep(Duration::from_secs(2));
    }

    maybe_value = sieve.get_copy(&key).unwrap();

    assert_eq!(maybe_value, None);
  }
}
