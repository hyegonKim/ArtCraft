use actix_web::HttpRequest;
use http_server_common::request::get_request_header_optional::get_request_header_optional;

// Cloudflare Ray ID HTTP header name
// https://developers.cloudflare.com/fundamentals/reference/cloudflare-ray-id/
const CLOUDFLARE_RAY_HEADER: &str = "cf-ray";

pub fn get_cloudflare_ray_header(request: &HttpRequest) -> Option<String> {
  get_request_header_optional(request, CLOUDFLARE_RAY_HEADER)
}
