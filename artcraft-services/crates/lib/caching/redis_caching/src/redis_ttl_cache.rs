use r2d2::{Pool, PooledConnection};
use redis::{Client, Commands};
use serde::{Deserialize, Serialize};
use std::future::Future;

use errors::AnyhowResult;

const DEFAULT_TTL_SECONDS : u64 = 60;

const REDIS_KEY_PREFIX : &str = "_c";

// TODO: Async Redis

// TODO: This *really* needs tests.

#[derive(Clone)]
pub struct RedisTtlCache {
  redis_pool: Pool<Client>,
  default_key_ttl: u64,
}

impl RedisTtlCache {
  pub fn new(redis_pool: Pool<Client>) -> Self {
    Self::new_with_ttl(redis_pool, DEFAULT_TTL_SECONDS)
  }

  pub fn new_with_ttl(redis_pool: Pool<Client>, key_ttl: u64) -> Self {
    Self {
      redis_pool,
      default_key_ttl: key_ttl,
    }
  }

  pub fn get_connection(&self) -> AnyhowResult<RedisTtlCacheConnection> {
    let redis = self.redis_pool.get()?;
    Ok(RedisTtlCacheConnection {
      redis_connection: redis,
      default_key_ttl: self.default_key_ttl,
    })
  }
}

pub struct RedisTtlCacheConnection {
  redis_connection: PooledConnection<Client>,
  default_key_ttl: u64,
}

impl RedisTtlCacheConnection {

  pub fn delete_from_cache(&mut self, item_key: &str) -> AnyhowResult<()> {
    let redis_key = Self::cache_key(item_key);
    let _r : Option<String> = self.redis_connection.del(redis_key)?;
    Ok(())
  }

  pub fn get_from_cache<T>(&mut self, item_key: &str) -> AnyhowResult<Option<T>> where T: for <'a> Deserialize<'a> {
    let redis_key = Self::cache_key(item_key);
    let maybe_value : Option<String> = self.redis_connection.get(redis_key)?;

    if let Some(value) = maybe_value {
      let hydrated = serde_json::from_str::<T>(&value)?;
      return Ok(Some(hydrated));
    }

    Ok(None)
  }

  pub fn persist_to_cache<T>(&mut self, item_key: &str, item: T) -> AnyhowResult<()> where T: Serialize {
    let redis_key = Self::cache_key(item_key);
    let value = serde_json::to_string(&item)?;
    let _r : Option<String> = self.redis_connection.set_ex(redis_key, value, self.default_key_ttl)?;
    Ok(())
  }

  // NB(bt,2023-03-11): As of this writing, async closures are not yet stable.
  // Return closures with internal async blocks instead. See: https://stackoverflow.com/a/74004017
  pub async fn lazy_load_if_not_cached<T, F, Fut>(&mut self, item_key: &str, loader_func: F)
    -> AnyhowResult<Option<T>>
    where for<'a> T: Serialize + Deserialize<'a> + Clone,
          F: FnOnce() -> Fut,
          Fut: Future<Output=AnyhowResult<Option<T>>>,
  {
    let maybe_value = self.get_from_cache(item_key)?;

    if let Some(value) = maybe_value {
      return Ok(value);
    }

    let maybe_value = loader_func().await?;

    if let Some(value) = maybe_value {
      self.persist_to_cache(item_key, value.clone())?;
      return Ok(Some(value));
    }

    Ok(None)
  }

  fn cache_key(key: &str) -> String {
    format!("{}:{}", REDIS_KEY_PREFIX, key)
  }
}
