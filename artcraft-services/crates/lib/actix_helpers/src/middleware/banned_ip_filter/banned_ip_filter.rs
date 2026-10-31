use actix_web::dev::{Service, Transform};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::Error;
use futures_util::future::{ok, Ready};

use crate::middleware::banned_ip_filter::banned_ip_filter_middleware::BannedIpFilterMiddleware;
use crate::middleware::banned_ip_filter::ip_ban_list::ip_ban_list::IpBanList;

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.

#[derive(Clone)]
pub struct BannedIpFilter {
  ip_ban_list: IpBanList,
}

impl BannedIpFilter {
  pub fn new(ip_ban_list: IpBanList) -> Self {
    Self {
      ip_ban_list,
    }
  }
}

impl<S, B> Transform<S, ServiceRequest> for BannedIpFilter
  where
      S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
      S::Future: 'static,
{
  type Response = ServiceResponse<B>;
  type Error = Error;
  type InitError = ();
  type Transform = BannedIpFilterMiddleware<S>;
  type Future = Ready<Result<Self::Transform, Self::InitError>>;

  fn new_transform(&self, service: S) -> Self::Future {
    ok(BannedIpFilterMiddleware { service, ip_ban_list: self.ip_ban_list.clone() })
  }
}
