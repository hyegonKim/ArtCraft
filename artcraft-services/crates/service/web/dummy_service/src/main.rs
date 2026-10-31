//! This service is meant to help with debugging.

use std::sync::Arc;

use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::{App, Error, HttpServer, web};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::{Compress, DefaultHeaders, Logger};
use actix_web::web::Data;

use actix_cors_configs::cors::build_cors_config;
use actix_helpers::route_builder::RouteBuilder;
use errors::AnyhowResult;
use crate::endpoints::dummy_app_state_handler::dummy_app_state_handler;
use crate::endpoints::dummy_health_check_handler::dummy_health_check_handler;
use crate::endpoints::dummy_queue_stats_handler::dummy_queue_stats_handler;
use crate::endpoints::root_handler::root_handler;
use crate::endpoints::simple_pushback_handler::simple_pushback_handler;
use crate::endpoints::status_alert_handler::status_alert_handler;
use crate::env_args::env_args;
use crate::server_state::ServerState;

pub mod endpoints;
pub mod env_args;
pub mod server_state;

pub const DEFAULT_RUST_LOG: &str = concat!(
  "debug,",
  "actix_web=info,",
  "hyper::proto::h1::io=warn,",
  "http_server_common::request::get_request_ip=info," // Debug spams Rust logs
);

pub const LOG_FORMAT : &str =
  "[dummy-service] [%{HOSTNAME}e] %{X-Forwarded-For}i \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T";

#[actix_web::main]
async fn main() -> AnyhowResult<()> {
  easyenv::init_all_with_default_logging(Some(DEFAULT_RUST_LOG));

  //let log_format = "[%{HOSTNAME}e] %a \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T";

  let env_args = env_args()?;

  let server_state = ServerState::build(&env_args);
  let server_hostname = server_state.hostname.clone();

  // NB(bt,2024-03-24): This type is supposed to be deprecated.
  let old_server_environment = env_args.server_environment;

  // TODO: Fix duplication for gzip compression. This is stupid.
  //  I'm too tired to figure out the generic types though.
  if env_args.enable_gzip {
    HttpServer::new(move || {
      let app = App::new()
          .app_data(Data::new(Arc::new(server_state.clone())))
          .wrap(build_cors_config(old_server_environment))
          .wrap(Logger::new(LOG_FORMAT))
          .wrap(DefaultHeaders::new()
              .add(("X-Backend-Hostname", server_hostname.as_str())))
          .wrap(Compress::default());

      build_routes(app)
    })
        .bind(&env_args.bind_address)?
        .workers(env_args.num_workers)
        .run()
        .await?;
  } else {
    HttpServer::new(move || {
      let app = App::new()
          .app_data(Data::new(Arc::new(server_state.clone())))
          .wrap(build_cors_config(old_server_environment))
          .wrap(Logger::new(LOG_FORMAT))
          .wrap(DefaultHeaders::new()
              .add(("X-Backend-Hostname", server_hostname.as_str())));

      build_routes(app)
    })
        .bind(&env_args.bind_address)?
        .workers(env_args.num_workers)
        .run()
        .await?;
  }

  Ok(())
}

fn build_routes<T, B> (app: App<T>) -> App<T>
  where
      B: MessageBody,
      T: ServiceFactory<
        ServiceRequest,
        Config = (),
        Response = ServiceResponse<B>,
        Error = Error,
        InitError = (),
      >,
{
  RouteBuilder::from_app(app)
      .add_get("/", root_handler)
      .add_get("/_status", dummy_health_check_handler)
      .add_get("/v1/app_state", dummy_app_state_handler)
      .add_get("/v1/stats/queues", dummy_queue_stats_handler)
      .add_get("/v1/status_alert_check", status_alert_handler)
      .into_app()
      .default_service(web::route().to(simple_pushback_handler))
}