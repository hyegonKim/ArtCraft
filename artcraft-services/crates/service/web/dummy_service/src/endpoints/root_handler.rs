use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use actix_web::web::Data;

use crate::server_state::ServerState;

pub async fn root_handler(server_state: Data<Arc<ServerState>>) -> HttpResponse {
  let body = format!(" \
      <h1>We're Sorry! We appear to be having an outage!</h1> \
      <p>Wondering what's up? <a href=\"https://discord.gg/fakeyou\">Join our Discord to find out!</a></p> \
      <p>Maybe you want to work with us? We can pay! Get in touch!</p> \
      <p><em>storyteller-web instance: {}</em></p> \
      ", &server_state.hostname);
  HttpResponse::build(StatusCode::OK)
      .content_type("text/html; charset=utf-8")
      .body(body)
}
