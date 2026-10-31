use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{web, App, Error, HttpResponse};

use crate::http_server::endpoints::model_download::enqueue_gptsovits_model_download_handler::enqueue_gptsovits_model_download_handler;

pub fn add_model_download_routes<T, B>(app: App<T>) -> App<T>
where
    B: MessageBody,
    T: ServiceFactory<
      ServiceRequest,
      Config = (),
      Response = ServiceResponse<B>,
      Error = Error,
      InitError = ()
    >
{
  app.service(
    web
    ::scope("/v1/model_download")
        .service(web::resource("/gsv")
            .route(web::head().to(|| HttpResponse::Ok()))
            .route(web::post().to(enqueue_gptsovits_model_download_handler))
        )
  )
}
