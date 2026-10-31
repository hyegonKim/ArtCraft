use std::ops::Add;

use chrono::Utc;
use r2d2::PooledConnection;
use redis::{Client, Commands};

use crate::keys::premium::premium_avt_redis_key::PremiumAvtRedisKey;
use crate::keys::premium::premium_user_redis_key::PremiumUserRedisKey;
use crate::payloads::premium::inner_state::premium_payload::PremiumPayload;
use crate::traits::hkey_store_adapter::HkeyStoreAdapter;
use errors::AnyhowResult;

pub struct PremiumAvtPayload {
  pub key: PremiumAvtRedisKey,
  pub payload: PremiumPayload,
}

impl PremiumAvtPayload {

  pub fn new(key: PremiumAvtRedisKey) -> Self {
    Self {
      key,
      payload: PremiumPayload::new(),
    }
  }

  pub fn read_from_redis(key: PremiumAvtRedisKey, redis: &mut PooledConnection<Client>) -> AnyhowResult<Self> {
    let payload = PremiumPayload::read_from_redis(key.as_str(), redis)?;
    Ok(Self {
      key,
      payload,
    })
  }

  pub fn persist_to_redis_with_expiry(&self, redis: &mut PooledConnection<Client>) -> AnyhowResult<()> {
    self.payload.persist_to_redis(self.key.as_str(), redis)?;
    self.set_key_expiry(redis)?;
    Ok(())
  }

  pub fn set_key_expiry(&self, redis: &mut PooledConnection<Client>) -> AnyhowResult<()> {
    let expire_at= Utc::now()
        .add(PremiumUserRedisKey::get_redis_ttl())
        .timestamp() as i64;
    redis.expire_at::<_, ()>(self.key.as_str(), expire_at)?;
    Ok(())
  }
}
