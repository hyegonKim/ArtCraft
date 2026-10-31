use std::net::IpAddr;
use std::str::FromStr;

use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::dev::Service;
use actix_web::Error;
use futures_util::future::{Either, err, Ready};

use crate::extractors::get_service_request_ip_address::get_service_request_ip_address;
use crate::middleware::banned_cidr_filter::banned_cidr_set::BannedCidrSet;
use crate::middleware::banned_ip_filter::banned_error::BannedError;

//use std::task::{Context, Poll};

pub struct BannedCidrFilterMiddleware<S> {
  pub (crate) service: S,
  pub (crate) cidr_bans: BannedCidrSet,
}

impl<S, B> Service<ServiceRequest> for BannedCidrFilterMiddleware<S>
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
    let ip_address = match IpAddr::from_str(&ip_address) {
      Ok(ip) => ip,
      Err(_err) => {
        return Either::Left(self.service.call(request));
      }
    };

    // NB: Fail open again.
    let is_banned = self.cidr_bans
        .ip_is_banned(ip_address)
        .unwrap_or(false);

    if is_banned {
      Either::Right(err(Error::from(BannedError {})))
    } else {
      Either::Left(self.service.call(request))
    }
  }
}
