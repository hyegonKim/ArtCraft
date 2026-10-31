use std::collections::HashMap;

use r2d2::PooledConnection;
use redis::{Client, Commands};

use errors::AnyhowResult;

pub trait HkeyStoreAdapter {

  fn read_from_redis(redis_key: &str, redis: &mut PooledConnection<Client>) -> AnyhowResult<Self> where Self: Sized {
    let usage_map : HashMap<String, String> = redis.hgetall(redis_key)?;
    let hydrated = Self::hydrate_from_vec(usage_map)?;
    Ok(hydrated)
  }

  fn persist_to_redis(&self, redis_key: &str, redis: &mut PooledConnection<Client>) -> AnyhowResult<()> {
    let map = self.serialize_payload()?;
    redis.hset_multiple::<_, _, _, ()>(redis_key, &map)?;
    Ok(())
  }

  fn serialize_payload(&self) -> AnyhowResult<Vec<(String, String)>>;

  fn hydrate_from_vec(values: HashMap<String, String>) -> AnyhowResult<Self> where Self: Sized;
}
