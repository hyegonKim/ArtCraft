use crate::http_server::endpoints::analytics::log_app_active_user_handler::log_app_active_user_handler;
use crate::http_server::endpoints::analytics::log_app_active_user_json_handler::log_app_active_user_json_handler;
use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::Error;
use actix_web::{web, App, HttpResponse};

pub fn add_analytics_routes<T, B> (app: App<T>) -> App<T>
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
  app.service(web::scope("/v1/analytics")
      .service(web::resource("/active_user")
          .route(web::post().to(log_app_active_user_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/active_user_v2")
          .route(web::post().to(log_app_active_user_json_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
  )
}
