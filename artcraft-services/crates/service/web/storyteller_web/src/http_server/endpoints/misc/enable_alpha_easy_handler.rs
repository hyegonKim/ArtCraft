use std::ops::Deref;
use std::sync::Arc;

use actix_web::http::header::LOCATION;
use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use log::info;

use crate::http_server::endpoints::misc::alpha_cookie::alpha_cookie;
use crate::state::server_state::ServerState;

#[get("/enable")]
pub async fn enable_alpha_easy_handler(
  http_request: HttpRequest,
  server_state: web::Data<Arc<ServerState>>
) -> impl Responder
{
  info!("GET /enable");

  let cookie = alpha_cookie(&server_state);

  let response = HttpResponse::Found()
      .cookie(cookie)
      .header(LOCATION, server_state.env_config.website_homepage_redirect.deref())
      .finish();

  response
}
