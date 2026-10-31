use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{web, App, Error, HttpResponse};

use crate::http_server::endpoints::beta_keys::create_beta_keys_handler::create_beta_keys_handler;
use crate::http_server::endpoints::beta_keys::edit_beta_key_distributed_flag_handler::edit_beta_key_distributed_flag_handler;
use crate::http_server::endpoints::beta_keys::edit_beta_key_note_handler::edit_beta_key_note_handler;
use crate::http_server::endpoints::beta_keys::list_beta_keys_handler::list_beta_keys_handler;
use crate::http_server::endpoints::beta_keys::redeem_beta_key_handler::redeem_beta_key_handler;

pub fn add_beta_key_routes<T, B> (app: App<T>) -> App<T>
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
  app.service(web::scope("/v1/beta_keys")
      .service(web::resource("/list")
          .route(web::get().to(list_beta_keys_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/create")
          .route(web::post().to(create_beta_keys_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/redeem")
          .route(web::post().to(redeem_beta_key_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/{token}/note")
          .route(web::post().to(edit_beta_key_note_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/{token}/distributed")
          .route(web::post().to(edit_beta_key_distributed_flag_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
  )
}
