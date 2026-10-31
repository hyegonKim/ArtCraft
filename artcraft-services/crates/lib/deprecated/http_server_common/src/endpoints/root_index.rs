use actix_http::StatusCode;
use actix_web::HttpResponse;
use log::debug;

pub async fn get_root_index() -> HttpResponse {
  debug!("GET /"); // NB: Google load balancer hits this a lot, and it spams.
  HttpResponse::build(StatusCode::OK)
    .content_type("text/html; charset=utf-8")
    .body("<h1>hello!</h1><p>Are you looking for an API? Join our Discord!</p><p>Maybe you want to work with us? We can pay! Get in touch!</p>")
}
