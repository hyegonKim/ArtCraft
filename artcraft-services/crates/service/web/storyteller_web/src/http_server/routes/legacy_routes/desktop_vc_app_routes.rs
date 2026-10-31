use crate::http_server::deprecated_endpoints::stubs::app_model_downloads::get_app_model_downloads_handler;
use crate::http_server::deprecated_endpoints::stubs::app_news::get_app_news_handler;
use crate::http_server::deprecated_endpoints::stubs::app_plans::get_app_plans_handler;
use crate::http_server::deprecated_endpoints::stubs::post_app_analytics::post_app_analytics_handler;
use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::Error;
use actix_web::{web, App, HttpResponse};

pub fn add_desktop_vc_app_routes<T, B> (app: App<T>) -> App<T>
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
  app.service(web::scope("/v1/vc")
      .service(web::resource("/report_analytics")
          .route(web::post().to(post_app_analytics_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/news")
          .route(web::get().to(get_app_news_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/features")
          .route(web::get().to(get_app_plans_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/downloads")
          .route(web::get().to(get_app_model_downloads_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
  )
}
