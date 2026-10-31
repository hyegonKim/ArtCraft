use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::api_defs::users::login::{LoginErrorResponse, LoginRequest, LoginSuccessResponse};
use crate::error::api_error::ApiError;
use crate::error::storyteller_error::StorytellerError;
use crate::utils::api_host::ApiHost;
use crate::utils::api_or_web_json_post_request::api_or_web_json_post_request;

pub const LOGIN_PATH: &str = "/v1/login";

pub struct LoginArgs<'a> {
  pub api_host: &'a ApiHost,
  pub request: &'a LoginRequest,
}

/// Failure modes of the login endpoint.
#[derive(Debug)]
pub enum LoginError {
  /// The API rejected the login with a structured error (invalid
  /// credentials, account needs a password, server error).
  Login(LoginErrorResponse),
  /// Transport, client, or unexpected server failure.
  Storyteller(StorytellerError),
}

/// Log in with a username (or email) and password. On success the returned
/// `signed_session` can be sent back as a `session=<value>` cookie.
pub async fn login(
  args: LoginArgs<'_>,
) -> Result<LoginSuccessResponse, LoginError> {
  api_or_web_json_post_request(
    args.api_host,
    LOGIN_PATH,
    None,
    args.request,
  ).await.map_err(classify_error)
}

/// The login endpoint returns its structured error body on 401/500; surface
/// it as a typed [`LoginErrorResponse`] when it parses, and fall back to the
/// raw transport error otherwise.
fn classify_error(error: StorytellerError) -> LoginError {
  let maybe_body = match &error {
    StorytellerError::Api(ApiError::Unauthorized(body)) => Some(body),
    StorytellerError::Api(ApiError::InternalServerError { body, .. }) => Some(body),
    _ => None,
  };

  let maybe_response = maybe_body
      .and_then(|body| serde_json::from_str::<LoginErrorResponse>(body).ok());

  match maybe_response {
    Some(response) => LoginError::Login(response),
    None => LoginError::Storyteller(error),
  }
}

impl Error for LoginError {}

impl Display for LoginError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      LoginError::Login(response) => {
        write!(f, "Login error ({:?}): {}", response.error_type, response.error_message)
      }
      LoginError::Storyteller(error) => write!(f, "{}", error),
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::api_defs::users::login::LoginErrorType;
  use crate::error::api_error::ApiError;
  use crate::error::storyteller_error::StorytellerError;
  use super::*;

  mod classify_error_tests {
    use super::*;

    #[test]
    fn unauthorized_with_login_body_is_typed() {
      let body = r#"{"success":false,"error_type":"InvalidCredentials","error_message":"invalid credentials"}"#;
      let error = StorytellerError::Api(ApiError::Unauthorized(body.to_string()));

      match classify_error(error) {
        LoginError::Login(response) => {
          assert_eq!(response.error_type, LoginErrorType::InvalidCredentials);
          assert_eq!(response.error_message, "invalid credentials");
        }
        other => panic!("expected typed login error, got {:?}", other),
      }
    }

    #[test]
    fn server_error_with_login_body_is_typed() {
      let body = r#"{"success":false,"error_type":"ServerError","error_message":"server error"}"#;
      let error = StorytellerError::Api(ApiError::InternalServerError {
        body: body.to_string(),
        backend_hostname: None,
      });

      match classify_error(error) {
        LoginError::Login(response) => {
          assert_eq!(response.error_type, LoginErrorType::ServerError);
        }
        other => panic!("expected typed login error, got {:?}", other),
      }
    }

    #[test]
    fn unparsable_body_falls_back_to_transport_error() {
      let error = StorytellerError::Api(ApiError::Unauthorized("<html>gateway</html>".to_string()));

      match classify_error(error) {
        LoginError::Storyteller(_) => {}
        other => panic!("expected transport error, got {:?}", other),
      }
    }
  }
}
