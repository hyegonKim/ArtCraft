use actix_web::dev::{Service, Transform};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::Error;
use futures_util::future::{ok, Ready};

use crate::middleware::disabled_endpoint_filter::disabled_endpoint_filter_middleware::DisabledEndpointFilterMiddleware;
use crate::middleware::disabled_endpoint_filter::disabled_endpoints::disabled_endpoints::DisabledEndpoints;

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.

#[derive(Clone)]
pub struct DisabledEndpointFilter {
  disabled_endpoints: DisabledEndpoints,
}

impl DisabledEndpointFilter {
  pub fn new(disabled_endpoints: DisabledEndpoints) -> Self {
    Self {
      disabled_endpoints,
    }
  }
}

impl<S, B> Transform<S, ServiceRequest> for DisabledEndpointFilter
  where
      S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
      S::Future: 'static,
{
  type Response = ServiceResponse<B>;
  type Error = Error;
  type InitError = ();
  type Transform = DisabledEndpointFilterMiddleware<S>;
  type Future = Ready<Result<Self::Transform, Self::InitError>>;

  fn new_transform(&self, service: S) -> Self::Future {
    ok(DisabledEndpointFilterMiddleware { service, disabled_endpoints: self.disabled_endpoints.clone() })
  }
}
