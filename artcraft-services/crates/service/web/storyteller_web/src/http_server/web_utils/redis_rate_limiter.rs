//! NB: This doesn't live in the `util.redis` package because it depends on `http_server`.
//! It's only used for the server anyway, so living here makes sense.

use actix_http::header::HeaderName;
use actix_web::HttpRequest;
use futures_old_for_limiter::Future;
use log::info;

use http_server_common::request::get_request_ip::get_request_ip;
use limitation::{Error, Limiter};

/// If this HTTP header is set, the rate limiter can be bypassed (eg. for debugging)
const RATE_LIMIT_BYPASS_HEADER : &str = "limitless";

#[derive(Clone)] // NB: Limiter is `Clone`
pub struct RedisRateLimiter {
  limiter: Limiter,
  limit_key_prefix: String,
  limiter_enabled: bool,
  rate_limit_bypass_header: HeaderName,
}

pub enum RateLimiterError {
  RateLimitExceededError,
  ClientError, // NB: Fail open for this!
}

impl RedisRateLimiter {
  pub fn new(limiter: Limiter, limit_key_prefix: &str, limiter_enabled: bool) -> Self {
    let rate_limit_bypass_header = HeaderName::from_static(RATE_LIMIT_BYPASS_HEADER);
    RedisRateLimiter {
      limiter,
      limit_key_prefix: limit_key_prefix.to_string(),
      limiter_enabled,
      rate_limit_bypass_header,
    }
  }

  pub async fn rate_limit_request(&self, request: &HttpRequest) -> Result<(), RateLimiterError> {
    if !self.limiter_enabled {
      return Ok(())
    }

    let headers = request.headers();
    if headers.contains_key(&self.rate_limit_bypass_header) {
      info!("Bypassing rate limiter with special bypass/debug header.");
      return Ok(());
    }

    let ip_address = get_request_ip(&request);

    let rate_limit_key = format!("rate_limit:{}:{}", &self.limit_key_prefix, ip_address);
    self.rate_limit_key(&rate_limit_key).await
  }

  pub async fn rate_limit_key(&self, rate_limit_key: &str) -> Result<(), RateLimiterError> {
    if !self.limiter_enabled {
      return Ok(())
    }

    // NB: This library uses old-school futures (pre-async/await)
    let permit = self.limiter.count(rate_limit_key).await;

    // TODO/FIXME: This could block.
    match permit {
      Ok(_) => Ok(()),
      Err(err) => match err {
        Error::Client(_) => Ok(()), // NB: Fail open for failure to connect to Redis
        Error::Time(_) => Ok(()), // NB: Fail open for key parsing
        Error::ChronoTime(_) => Ok(()), // NB: Fail open for key parsing
        Error::LimitExceeded(_) => Err(RateLimiterError::RateLimitExceededError),
      },
    }
  }
}
