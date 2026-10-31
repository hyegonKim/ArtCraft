use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{web, App, Error, HttpResponse};

use crate::http_server::endpoints::weights::delete::delete_weight_handler::delete_weight_handler;
use crate::http_server::endpoints::weights::get::get_weight_handler::get_weight_handler;
use crate::http_server::endpoints::weights::list::list_available_weights_handler::list_available_weights_handler;
use crate::http_server::endpoints::weights::list::list_featured_weights_handler::list_featured_weights_handler;
use crate::http_server::endpoints::weights::list::list_pinned_weights_handler::list_pinned_weights_handler;
use crate::http_server::endpoints::weights::list::list_weights_by_user_handler::list_weights_by_user_handler;
use crate::http_server::endpoints::weights::search::search_model_weights_http_get_handler::search_model_weights_http_get_handler;
use crate::http_server::endpoints::weights::search::search_model_weights_http_post_handler::search_model_weights_http_post_handler;
use crate::http_server::endpoints::weights::update::set_model_weight_cover_image_handler::set_model_weight_cover_image_handler;
use crate::http_server::endpoints::weights::update::update_weight_handler::update_weight_handler;

pub fn add_weights_routes<T, B>(app: App<T>) -> App<T>
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
    ::scope("/v1/weights")
        .service(web::resource("/weight/{weight_token}")
            .route(web::get().to(get_weight_handler))
            .route(web::post().to(update_weight_handler))
            .route(web::delete().to(delete_weight_handler))
        )
        .service(web::resource("/search")
            .route(web::get().to(search_model_weights_http_get_handler))
            .route(web::post().to(search_model_weights_http_post_handler))
            .route(web::head().to(|| HttpResponse::Ok()))
        )
        .service(web::resource("/weight/{token}/cover_image")
            .route(web::post().to(set_model_weight_cover_image_handler))
        )
        .route("/by_user/{username}", web::get().to(list_weights_by_user_handler))
        .route("/list", web::get().to(list_available_weights_handler))
        .route("/list_featured", web::get().to(list_featured_weights_handler))
        .route("/list_pinned", web::get().to(list_pinned_weights_handler))
  )
}
