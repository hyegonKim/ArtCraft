use crate::http_server::endpoints::user_bookmarks::batch_get_user_bookmarks_handler::batch_get_user_bookmarks_handler;
use crate::http_server::endpoints::user_bookmarks::create_user_bookmark_handler::create_user_bookmark_handler;
use crate::http_server::endpoints::user_bookmarks::delete_user_bookmark_handler::delete_user_bookmark_handler;
use crate::http_server::endpoints::user_bookmarks::list_user_bookmarks_for_entity_handler::list_user_bookmarks_for_entity_handler;
use crate::http_server::endpoints::user_bookmarks::list_user_bookmarks_for_user_handler::list_user_bookmarks_for_user_handler;
use actix_helpers::route_builder::RouteBuilder;
use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::Error;
use actix_web::App;

pub fn add_user_bookmarks_routes<T, B> (app: App<T>) -> App<T>
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
  RouteBuilder::from_app(app)
      .add_get("/v1/user_bookmarks/batch", batch_get_user_bookmarks_handler)
      .add_post("/v1/user_bookmarks/create", create_user_bookmark_handler)
      .add_post("/v1/user_bookmarks/delete/{user_bookmark_token}", delete_user_bookmark_handler)
      //.add_get("/v1/user_bookmarks/list/session", list_user_bookmarks_for_session_handler)
      .add_get("/v1/user_bookmarks/list/user/{username}", list_user_bookmarks_for_user_handler)
      .add_get("/v1/user_bookmarks/list/entity/{entity_type}/{entity_token}", list_user_bookmarks_for_entity_handler)
      .into_app()
}

