use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::dev::Service;
use actix_web::Error;
use futures_util::future::{Either, err, Ready};

use crate::extractors::get_service_request_ip_address::get_service_request_ip_address;
use crate::middleware::banned_ip_filter::banned_error::BannedError;
use crate::middleware::banned_ip_filter::ip_ban_list::ip_ban_list::IpBanList;

//use std::task::{Context, Poll};

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.

pub struct BannedIpFilterMiddleware<S> {
  pub (crate) service: S,
  pub (crate) ip_ban_list: IpBanList,
}

impl<S, B> Service<ServiceRequest> for BannedIpFilterMiddleware<S>
  where
      S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
      S::Future: 'static,
{
  type Response = ServiceResponse<B>;
  type Error = Error;
  type Future = Either<S::Future, Ready<Result<Self::Response, Self::Error>>>;

  //// alternatively(?), actix_service::forward_ready!(service);
  //fn poll_ready(&self, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
  //  self.service.poll_ready(cx)
  //}

  actix_service::forward_ready!(service);

  fn call(&self, request: ServiceRequest) -> Self::Future {
    let ip_address = get_service_request_ip_address(&request);

    // NB: Fail open.
    // We don't want our service to explode because we can't read bans.
    let is_banned = self.ip_ban_list
        .contains_ip_address(&ip_address)
        .unwrap_or(false);

    if is_banned {
      Either::Right(err(Error::from(BannedError {})))
    } else {
      Either::Left(self.service.call(request))
    }
  }
}
