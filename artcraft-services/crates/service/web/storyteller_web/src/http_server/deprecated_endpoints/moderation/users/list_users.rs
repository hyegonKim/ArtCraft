use std::fmt;
use std::sync::Arc;

use actix_web::error::ResponseError;
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use log::warn;

use mysql_queries::queries::users::user::list_users_query_builder::{ListUsersQueryBuilder, UserForList};

use crate::http_server::web_utils::serialize_as_json_error::serialize_as_json_error;
use crate::state::server_state::ServerState;

// TODO: Implement the endpoint
#[derive(Deserialize)]
pub struct ListUsersRequest;

#[derive(Serialize)]
pub struct ListUsersResponse {
  success: bool,
  users: Vec<UserForList>
}

#[derive(Serialize, Debug)]
pub struct ListUsersErrorResponse {
  pub success: bool,
  pub error_type: ListUsersErrorType,
  pub error_message: String,
}

#[derive(Copy, Clone, Debug, Serialize)]
pub enum ListUsersErrorType {
  BadInput,
  ServerError,
  Unauthorized,
  UserNotFound,
}

impl ListUsersErrorResponse {
  fn unauthorized() -> Self {
    Self {
      success: false,
      error_type: ListUsersErrorType::Unauthorized,
      error_message: "unauthorized".to_string()
    }
  }
  fn server_error() -> Self {
    Self {
      success: false,
      error_type: ListUsersErrorType::ServerError,
      error_message: "server error".to_string()
    }
  }
  fn bad_request(error_message: &str) -> Self {
    Self {
      success: false,
      error_type: ListUsersErrorType::BadInput,
      error_message: error_message.to_string()
    }
  }
}

impl ResponseError for ListUsersErrorResponse {
  fn status_code(&self) -> StatusCode {
    match self.error_type {
      ListUsersErrorType::BadInput => StatusCode::BAD_REQUEST,
      ListUsersErrorType::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
      ListUsersErrorType::Unauthorized => StatusCode::UNAUTHORIZED,
      ListUsersErrorType::UserNotFound => StatusCode::NOT_FOUND,
    }
  }

  fn error_response(&self) -> HttpResponse {
    serialize_as_json_error(self)
  }
}

// NB: Not using DeriveMore since Clion doesn't understand it.
impl fmt::Display for ListUsersErrorResponse {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self.error_type)
  }
}

pub async fn list_users_handler(
  http_request: HttpRequest,
  request: web::Query<ListUsersRequest>,
  server_state: web::Data<Arc<ServerState>>
) -> Result<HttpResponse, ListUsersErrorResponse> {

  let maybe_user_session = server_state
      .session_checker
      .maybe_get_user_session(&http_request, &server_state.mysql_pool)
      .await
      .map_err(|e| {
        warn!("Session checker error: {:?}", e);
        ListUsersErrorResponse::server_error()
      })?;

  let user_session = match maybe_user_session {
    Some(session) => session,
    None => {
      warn!("not logged in");
      return Err(ListUsersErrorResponse::unauthorized());
    }
  };

  // TODO: Not the correct permission
  if !user_session.can_ban_users {
    warn!("user is not allowed to add bans: {:?}", user_session.user_token);
    return Err(ListUsersErrorResponse::unauthorized());
  }

  let query_results = ListUsersQueryBuilder::new()
      .sort_ascending(true)
      .query_for_page(&server_state.mysql_pool)
      .await;

  let results_page = match query_results {
    Ok(results) => results,
    Err(e) => {
      warn!("Query error: {:?}", e);
      return Err(ListUsersErrorResponse::server_error());
    }
  };

  let response = ListUsersResponse {
    success: true,
    users: results_page.users,
  };

  let body = serde_json::to_string(&response)
      .map_err(|e| ListUsersErrorResponse::server_error())?;

  Ok(HttpResponse::Ok()
      .content_type("application/json")
      .body(body))
}
