use crate::http_server::endpoints::credits::get_session_credits_handler::get_session_credits_handler;
use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{web, App, Error, HttpResponse};

pub fn add_credits_routes<T, B> (app: App<T>) -> App<T>
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
  app.service(web::scope("/v1/credits")
      .service(web::resource("/namespace/{namespace}")
          .route(web::get().to(get_session_credits_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
  )
}
