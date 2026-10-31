use std::io::Write;

use actix_http::body::BoxBody;
use actix_http::header::HeaderName;
use actix_http::StatusCode;
use actix_web::dev::{Service, Transform};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::USER_AGENT;
use actix_web::{Error, HttpResponse};
use actix_web::{HttpMessage, ResponseError};
use futures_util::future::{err, ok, Either, Ready};
use log::warn;

use http_server_common::request::get_request_ip::get_service_request_ip;

use crate::http_server::web_utils::response_error_helpers::to_simple_json_error;
use crate::state::server_state::StaticFeatureFlags;

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.

#[derive(Debug)]
pub struct PushbackError;

impl std::fmt::Display for PushbackError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let response = "ERR429.67: too many requests";
    write!(f, "{}", response)
  }
}

impl std::error::Error for PushbackError {}

impl ResponseError for PushbackError {
  fn status_code(&self) -> StatusCode {
    StatusCode::TOO_MANY_REQUESTS
  }

  fn error_response(&self) -> HttpResponse<BoxBody> {
    // NB: I'm setting a string error code because I mistakenly got caught by this in local dev
    // and couldn't figure out the issue for a bit. At least I can grep for this string.
    // However, I need to balance this requirement with not cluing in those that are banned.
    to_simple_json_error(
      "ERR429.67: too many requests",
      self.status_code())
  }
}

#[derive(Clone)] // NB: Internal state must be Clone-safe (Arc, etc.)
pub struct PushbackFilter {
  feature_flags: StaticFeatureFlags,
}

impl PushbackFilter {
  pub fn new(feature_flags: &StaticFeatureFlags) -> Self {
    Self {
      feature_flags: feature_flags.clone(),
    }
  }
}

impl<S, B> Transform<S, ServiceRequest> for PushbackFilter
  where
      S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
      S::Future: 'static,
{
  type Response = ServiceResponse<B>;
  type Error = Error;
  type InitError = ();
  type Transform = PushbackFilterMiddleware<S>;
  type Future = Ready<Result<Self::Transform, Self::InitError>>;

  fn new_transform(&self, service: S) -> Self::Future {
    ok(PushbackFilterMiddleware { service, feature_flags: self.feature_flags.clone() })
  }
}

pub struct PushbackFilterMiddleware<S> {
  service: S,
  feature_flags: StaticFeatureFlags,
}

impl<S, B> Service<ServiceRequest> for PushbackFilterMiddleware<S>
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

  fn call(&self, req: ServiceRequest) -> Self::Future {
    // NB: Ordinarily the filter should be disabled.
    let mut can_bypass_filter = !self.feature_flags.global_429_pushback_filter_enabled;

    if !can_bypass_filter {
      // Don't kill the load balancer!
      //
      // - `/` is used by the K8s load balancer:
      //       gcloud compute health-checks describe k8s1-30d9a0ff-storyteller-storyteller-service-8080-0a8fb2fe
      //
      // - `/_status` is used by the K8s pod scheduler.
      //
      can_bypass_filter = req.path().starts_with("/_status") || req.path().eq("/");
    }

    if !can_bypass_filter {
      can_bypass_filter = is_google_http_load_balancer_health_check(&req) || is_kube_probe_health_check(&req);
    }

    if !can_bypass_filter {
      // TODO: Clean up with transpose() once stable
      let result = req.headers()
          .get("bypass-pushback-filter")
          .map(|h| h.to_str());

      can_bypass_filter = match result {
        Some(Ok(header)) => true,
        Some(Err(_)) => false,
        None => false,
      };
    }

    if can_bypass_filter {
      Either::Left(self.service.call(req))
    } else {
      // NB: Actix won't log the request when we return an error, so we manually log it.
      log_dropped_request(&req);
      Either::Right(err(Error::from(PushbackError {})))
    }
  }
}

fn log_dropped_request(request: &ServiceRequest) {
  // TODO: Is there a way to use the actix logger?
  let remote_ip_address = get_service_request_ip(request);
  let request_method = request.method().as_str();
  let request_path = request.path();
  let user_agent = get_header_value(request, &USER_AGENT).unwrap_or("");

  warn!("dropped request: {} {} {} {}", remote_ip_address, request_method, request_path, user_agent)
}

fn is_google_http_load_balancer_health_check(request: &ServiceRequest) -> bool {
  let is_load_balancer_user_agent = get_header_value(request, &USER_AGENT)
      .map(|user_agent| user_agent.starts_with("GoogleHC")) // eg "GoogleHC/1.0"
      .unwrap_or(false);

  // TODO/FIXME: This may be incorrect, but the load balancer IPs seem to start with 35.* or 130.*
  //  https://cloud.google.com/load-balancing/docs/https
  // We should only allow these tests to pass for those IP ranges.

  is_load_balancer_user_agent
}

fn is_kube_probe_health_check(request: &ServiceRequest) -> bool {
  let is_k8s_probe_user_agent = get_header_value(request, &USER_AGENT)
      .map(|user_agent| user_agent.starts_with("kube-probe")) // eg "kube-probe/1.23"
      .unwrap_or(false);

  // TODO/FIXME: Constrain to k8s IP ranges

  is_k8s_probe_user_agent
}

fn get_header_value<'a>(request: &'a ServiceRequest, header_name: &HeaderName) -> Option<&'a str> {
  request.headers()
      .get(header_name)
      .map(|h| h.to_str())
      .transpose()
      .unwrap_or(None)
}
