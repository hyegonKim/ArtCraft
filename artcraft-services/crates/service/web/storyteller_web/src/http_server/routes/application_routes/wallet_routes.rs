use crate::http_server::endpoints::user_wallets::get_or_create_session_artcraft_wallet_handler::get_or_create_session_artcraft_wallet_handler;
use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{web, App, Error, HttpResponse};

pub fn add_wallet_routes<T, B>(app: App<T>) -> App<T>
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
  app.service(web::scope("/v1/wallets")
      .service(web::resource("/session_artcraft_wallet")
          .route(web::get().to(get_or_create_session_artcraft_wallet_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
  )
}
