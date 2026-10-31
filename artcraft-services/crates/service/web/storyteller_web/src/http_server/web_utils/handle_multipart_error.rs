use actix_multipart::MultipartError;
use actix_web::Error;
use actix_web::HttpRequest;
use log::error;

pub fn handle_multipart_error(err: MultipartError, req: &HttpRequest) -> Error {
  error!("Multipart error: {}", err);
  Error::from(err)
}
