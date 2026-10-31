use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::Error;
use actix_web::{web, App, HttpResponse};

use crate::http_server::endpoints::user_ratings::batch_get_user_rating_handler::batch_get_user_rating_handler;
use crate::http_server::endpoints::user_ratings::get_user_rating_handler::get_user_rating_handler;
use crate::http_server::endpoints::user_ratings::set_user_rating_handler::set_user_rating_handler;


pub fn add_user_rating_routes<T, B> (app: App<T>) -> App<T>
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
  app.service(web::scope("/v1/user_rating")
      .service(web::resource("/batch")
          .route(web::get().to(batch_get_user_rating_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/rate")
          .route(web::post().to(set_user_rating_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/view/{entity_type}/{entity_token}")
          .route(web::get().to(get_user_rating_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
  )
}
