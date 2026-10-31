use actix_web::{HttpResponse, Responder};
use log::warn;

pub async fn default_route_404() -> impl Responder {
  warn!("404 not found");
  HttpResponse::NotFound()
    .content_type("text/html; charset=utf-8")
    .body("<h1>not found</h1>")
}
