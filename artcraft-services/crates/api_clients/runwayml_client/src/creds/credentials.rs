use crate::creds::jwt_bearer_token::JwtBearerToken;
use crate::error::api_error::ApiError;

#[derive(Clone)]
pub struct Credentials {
  pub jwt_bearer_token: Option<JwtBearerToken>,
}

impl Credentials {
  pub fn from_jwt_str(jwt_str: &str) -> Result<Self, ApiError> {
    let jwt_bearer_token = JwtBearerToken::new(jwt_str.to_string())?;
    Ok(Self { 
      jwt_bearer_token: Some(jwt_bearer_token),
    })
  }
}
