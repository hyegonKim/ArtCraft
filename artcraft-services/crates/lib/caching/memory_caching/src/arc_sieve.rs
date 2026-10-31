use std::hash::Hash;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail};
use sieve_cache::SieveCache;

use errors::AnyhowResult;

#[derive(Clone)]
pub struct ArcSieve<K: Eq + Hash + Clone, V: Clone + ?Sized> {
  cache: Arc<Mutex<SieveCache<K, V>>>,
}

impl <K: Eq + Hash + Clone, V: Clone + ?Sized> ArcSieve<K, V> {

  pub fn with_capacity(capacity: usize) -> AnyhowResult<Self> {
    let cache = SieveCache::new(capacity)
        .map_err(|err| anyhow!("could not create cache: {:?}", err))?;
    Ok(Self {
      cache: Arc::new(Mutex::new(cache)),
    })
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

  pub fn get_copy(&self, key: &K) -> AnyhowResult<Option<V>> {
    let maybe_copy = match self.cache.lock() {
      Err(e) => bail!("could not unlock mutex to read: {:?}", e),
      Ok(mut cache) => {
        cache.get(key).map(|inner| inner.clone())
      },
    };
    Ok(maybe_copy)
  }
}

#[cfg(test)]
mod tests {
  use crate::arc_sieve::ArcSieve;

  #[test]
  fn test_get_copy() {
    let sieve : ArcSieve<String, String> = ArcSieve::with_capacity(1).unwrap();
    let key = "key".to_string();
    let value = "value".to_string();

    sieve.store_copy(&key, &value).unwrap();
    let maybe_value = sieve.get_copy(&key).unwrap();
    assert_eq!(maybe_value, Some("value".to_string()));
  }
}
