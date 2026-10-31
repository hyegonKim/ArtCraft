use crate::http_server::endpoints::subscriptions::get_session_subscription_handler::get_session_subscription_handler;
use crate::http_server::endpoints::subscriptions::unsubscribe_reason::set_unsubscribe_reason_handler;
use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{web, App, Error, HttpResponse};

pub fn add_subscription_routes<T, B> (app: App<T>) -> App<T>
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
  app.service(web::scope("/v1/subscriptions")
    .service(web::resource("/unsubscribe_reason")
      .route(web::post().to(set_unsubscribe_reason_handler))
      .route(web::head().to(|| HttpResponse::Ok()))
    )
    .service(web::resource("/namespace/{namespace}")
      .route(web::get().to(get_session_subscription_handler))
      .route(web::head().to(|| HttpResponse::Ok()))
    )
  )
}
