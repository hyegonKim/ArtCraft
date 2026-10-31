use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::web::Data;
use actix_web::HttpResponse;
use log::debug;

use crate::state::server_state::ServerState;

pub async fn get_root_handler(server_state: Data<Arc<ServerState>>) -> HttpResponse {
  debug!("GET /"); // NB: Google load balancer hits this a lot, and it spams.
  let body = format!(" \
      <h1>hello!</h1> \
      <p>Are you looking for an API? <a href=\"https://discord.gg/artcraft\">Join our Discord!</a></p> \
      <p>Maybe you want to work with us? We can pay! Get in touch!</p> \
      <p><em>storyteller-web instance: {}</em></p> \
      ", server_state.hostname);
  HttpResponse::build(StatusCode::OK)
    .content_type("text/html; charset=utf-8")
    .body(body)
}
