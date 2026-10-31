use actix_web::dev::{Service, Transform};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::Error;
use futures_util::future::{ok, Ready};

use crate::middleware::banned_cidr_filter::banned_cidr_filter_middleware::BannedCidrFilterMiddleware;
use crate::middleware::banned_cidr_filter::banned_cidr_set::BannedCidrSet;

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.

#[derive(Clone)]
pub struct BannedCidrFilter {
  cidr_bans: BannedCidrSet,
}

impl BannedCidrFilter {
  pub fn new(cidr_bans: BannedCidrSet) -> Self {
    Self {
      cidr_bans,
    }
  }
}

impl<S, B> Transform<S, ServiceRequest> for BannedCidrFilter
  where
      S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
      S::Future: 'static,
{
  type Response = ServiceResponse<B>;
  type Error = Error;
  type InitError = ();
  type Transform = BannedCidrFilterMiddleware<S>;
  type Future = Ready<Result<Self::Transform, Self::InitError>>;

  fn new_transform(&self, service: S) -> Self::Future {
    ok(BannedCidrFilterMiddleware { service, cidr_bans: self.cidr_bans.clone() })
  }
}

