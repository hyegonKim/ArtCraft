use crate::http_server::endpoints::misc::default_route_404::default_route_404;
use crate::http_server::endpoints::misc::detect_locale_handler::detect_locale_handler;
use crate::http_server::endpoints::misc::get_root_handler::get_root_handler;
use crate::http_server::endpoints::service::health_check_handler::get_health_check_handler;
use crate::http_server::endpoints::service::public_info_handler::get_public_info_handler;
use crate::http_server::endpoints::service::status_alert_handler::status_alert_handler;
use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{web, App, Error, HttpResponse};

/// Add the core service routes.
pub fn add_service_routes<T, B> (app: App<T>) -> App<T>
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
  app
      .service(
        web::resource("/_status")
            .route(web::get().to(get_health_check_handler))
            .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(
        // TODO(bt,2023-01-21): Couldn't scope to /v1/, actix routing table might not like collision
        web::resource("/server_info")
            .route(web::get().to(get_public_info_handler))
            .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(
        web::resource("/detect_locale")
            .route(web::get().to(detect_locale_handler))
            .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(
        web::resource("/v1/status_alert_check")
            .route(web::get().to(status_alert_handler))
            .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/")
          .route(web::get().to(get_root_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .default_service( web::route().to(default_route_404))
}
