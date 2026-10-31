use std::fmt;
use std::sync::Arc;

use actix_web::error::ResponseError;
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use log::warn;

use mysql_queries::queries::w2l::w2l_templates::list_pending_w2l_templates::list_pending_w2l_templates;

use crate::http_server::web_utils::serialize_as_json_error::serialize_as_json_error;
use crate::state::server_state::ServerState;

#[derive(Serialize)]
pub struct GetPendingW2lTemplatesResponse {
  pub success: bool,
  pub templates: Vec<PendingW2lTemplate>,
}

#[derive(Serialize)]
pub struct PendingW2lTemplate {
  pub template_token: String,
  pub title: String,
  pub template_type: String,
  pub duration_millis: i32,
  pub frame_width: i32,
  pub frame_height: i32,
  pub creator_user_token: String,
  pub creator_username: String,
  pub creator_display_name: String,
  pub creator_gravatar_hash: String,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub enum GetPendingW2lTemplatesError {
  ServerError,
  Unauthorized,
}

impl ResponseError for GetPendingW2lTemplatesError {
  fn status_code(&self) -> StatusCode {
    match *self {
      GetPendingW2lTemplatesError::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
      GetPendingW2lTemplatesError::Unauthorized => StatusCode::UNAUTHORIZED,
    }
  }

  fn error_response(&self) -> HttpResponse {
    serialize_as_json_error(self)
  }
}

// NB: Not using DeriveMore since Clion doesn't understand it.
impl fmt::Display for GetPendingW2lTemplatesError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

pub async fn get_pending_w2l_templates_handler(
  http_request: HttpRequest,
  server_state: web::Data<Arc<ServerState>>
) -> Result<HttpResponse, GetPendingW2lTemplatesError> {

  let maybe_user_session = server_state
      .session_checker
      .maybe_get_user_session(&http_request, &server_state.mysql_pool)
      .await
      .map_err(|e| {
        warn!("Session checker error: {:?}", e);
        GetPendingW2lTemplatesError::ServerError
      })?;

  let user_session = match maybe_user_session {
    Some(session) => session,
    None => {
      warn!("not logged in");
      return Err(GetPendingW2lTemplatesError::Unauthorized);
    }
  };

  if !user_session.can_approve_w2l_templates {
    warn!("user is not allowed to approve w2l templates: {:?}", user_session.user_token);
    return Err(GetPendingW2lTemplatesError::Unauthorized);
  }

  let results = list_pending_w2l_templates(&server_state.mysql_pool)
      .await
      .map_err(|err| {
        warn!("list pending w2l db error: {:?}", err);
        GetPendingW2lTemplatesError::ServerError
      })?;

  let results = results.into_iter().map(|r| {
    PendingW2lTemplate {
      template_token: r.template_token,
      title: r.title,
      template_type: r.template_type,
      duration_millis: r.duration_millis,
      frame_width: r.frame_width,
      frame_height: r.frame_height,
      creator_user_token: r.creator_user_token,
      creator_username: r.creator_username,
      creator_display_name: r.creator_display_name,
      creator_gravatar_hash: r.creator_gravatar_hash,
      created_at: r.created_at,
    }
  }).collect::<Vec<PendingW2lTemplate>>();

  let response = GetPendingW2lTemplatesResponse {
    success: true,
    templates: results,
  };

  let body = serde_json::to_string(&response)
      .map_err(|e| GetPendingW2lTemplatesError::ServerError)?;

  Ok(HttpResponse::Ok()
      .content_type("application/json")
      .body(body))
}

