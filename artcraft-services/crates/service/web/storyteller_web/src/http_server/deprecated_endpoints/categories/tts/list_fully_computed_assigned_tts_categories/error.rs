//// NB: Incrementally getting rid of build warnings...
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};

use http_server_common::response::serialize_as_json_error::serialize_as_json_error;

#[derive(Debug, Serialize)]
pub enum ListFullyComputedAssignedTtsCategoriesError {
  ServerError,
}

impl ResponseError for ListFullyComputedAssignedTtsCategoriesError {
  fn status_code(&self) -> StatusCode {
    match *self {
      ListFullyComputedAssignedTtsCategoriesError::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }

  fn error_response(&self) -> HttpResponse {
    serialize_as_json_error(self)
  }
}

// NB: Not using derive_more::Display since Clion doesn't understand it.
impl std::fmt::Display for ListFullyComputedAssignedTtsCategoriesError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self)
  }
}
