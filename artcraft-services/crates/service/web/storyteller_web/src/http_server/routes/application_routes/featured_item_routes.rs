use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::Error;
use actix_web::{web, App, HttpResponse};

use crate::http_server::endpoints::featured_items::create_featured_item_handler::create_featured_item_handler;
use crate::http_server::endpoints::featured_items::delete_featured_item_handler::delete_featured_item_handler;
use crate::http_server::endpoints::featured_items::get_is_featured_item_handler::get_is_featured_item_handler;

// ==================== FEATURED ITEM ROUTES ====================

pub fn add_featured_item_routes<T, B> (app: App<T>) -> App<T>
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
  app.service(web::scope("/v1/featured_item")
      .service(web::resource("/create")
          .route(web::post().to(create_featured_item_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/delete")
          .route(web::delete().to(delete_featured_item_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/is_featured/{entity_type}/{entity_token}")
          .route(web::get().to(get_is_featured_item_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
  )
}
