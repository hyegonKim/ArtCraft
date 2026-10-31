use crate::http_server::endpoints::comments::create_comment_handler::create_comment_handler;
use crate::http_server::endpoints::comments::delete_comment_handler::delete_comment_handler;
use crate::http_server::endpoints::comments::list_comments_handler::list_comments_handler;
use actix_helpers::route_builder::RouteBuilder;
use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::Error;
use actix_web::App;

pub fn add_comments_routes<T, B> (app: App<T>) -> App<T>
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
      .add_get("/v1/comments/list/{entity_type}/{entity_token}", list_comments_handler)
      .add_post("/v1/comments/new", create_comment_handler)
      .add_post("/v1/comments/delete/{comment_token}", delete_comment_handler)
      .into_app()
}

