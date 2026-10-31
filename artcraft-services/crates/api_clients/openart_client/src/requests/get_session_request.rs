use crate::creds::openart_credentials::OpenArtCredentials;
use crate::error::api_error::ApiError;
use crate::error::classify_http_error_status_code_and_body::classify_http_error_status_code_and_body;
use crate::error::client_error::ClientError;
use crate::error::openart_error::OpenArtError;
use chrono::{DateTime, Utc};
use log::{debug, error, info};
use reqwest::Client;
use serde_derive::Deserialize;

const SESSION_URL : &str = "https://openart.ai/api/auth/session";

#[derive(Debug, Clone)]
pub struct SessionDetails {
  /// This is either a session ID, user ID, or subscription ID.
  /// It is passed as the header `X-USER-ID` in other requests.
  pub sub: Option<String>,

  pub email: Option<String>,
  pub name: Option<String>,
  pub image: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawSession {
  user: Option<RawUser>,
  expires: Option<DateTime<Utc>>,
  /// This is either a session ID, user ID, or subscription ID.
  /// It is passed as the header `X-USER-ID` in other requests.
  sub: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawUser {
  name: Option<String>,
  email: Option<String>,
  image: Option<String>,
}

pub async fn get_session_request(creds: &OpenArtCredentials) -> Result<SessionDetails, OpenArtError> {

  let cookies = match creds.cookies.as_ref() {
    Some(cookies) => cookies,
    None => {
      error!("Failed to request session. No cookies in credentials.");
      return Err(ClientError::NoCookiesInCredentials.into());
    }
  };

  let client = Client::builder()
      .gzip(true)
      .build()
      .map_err(|err| {
        error!("Failed to create HTTP client: {}", err);
        OpenArtError::Client(ClientError::ReqwestError(err))
      })?;

  info!("Getting session details from cookies... (cookie payload length: {})", cookies.as_str().len());

  let mut http_request= client.get(SESSION_URL)
      .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:137.0) Gecko/20100101 Firefox/137.0")
      .header("Accept", "*/*")
      .header("Accept-Encoding", "gzip, deflate, br")
      .header("Accept-Language", "en-US,en;q=0.5")
      .header("Cookie", cookies.as_str());

  let http_request = http_request.build()
      .map_err(|err| ApiError::ReqwestError(err))?;

  let response = client.execute(http_request)
      .await
      .map_err(|err| ApiError::ReqwestError(err))?;

  let status = response.status();

  let response_body = &response.text()
      .await
      .map_err(|err| {
        error!("Error reading body while attempting to read session details: {:?}", err);
        ApiError::ReqwestError(err)
      })?;

  if !status.is_success() {
    error!("Failed to get session details: {} ; body = {}", status, response_body);
    let error = classify_http_error_status_code_and_body(status, &response_body).await;
    return Err(error);
  }

  debug!("Session info response was 200. Body: {}", response_body);
  
  if response_body.is_empty() || response_body == "{}" {
    error!("Received empty response body when requesting session details.");
    return Err(OpenArtError::Api(ApiError::InvalidSession));
  }

  let session : RawSession = serde_json::from_str(response_body)
      .map_err(|err| { 
        error!("Failed to parse session details: {} body: {}", err, response_body);
        OpenArtError::Api(ApiError::CouldNotParseSession { error: err, body: response_body.to_string() } ) 
      })?;

  Ok(SessionDetails {
    sub: session.sub,
    email: session.user.as_ref().and_then(|u| u.email.clone()),
    name: session.user.as_ref().and_then(|u| u.name.clone()),
    image: session.user.as_ref().and_then(|u| u.image.clone()),
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::creds::openart_cookies::OpenArtCookies;
  use crate::creds::openart_credentials::OpenArtCredentials;

  #[tokio::test]
  #[ignore] // Do not run in CI. Run manully to test session retrieval.
  async fn invalid_session() {
    let creds = OpenArtCredentials {
      cookies: Some(OpenArtCookies::new("".to_string())),
      session_info: None,
    };

    let result = get_session_request(&creds).await;

    println!("Result: {:?}", result);

    assert!(result.is_err());
  }
  
  #[tokio::test]
  #[ignore] // Do not run in CI. Run manully to test session retrieval.
  async fn valid_session() {
    let cookie = "";
    let creds = OpenArtCredentials {
      cookies: Some(OpenArtCookies::new(cookie.to_string())),
      session_info: None,
    };

    let result = get_session_request(&creds).await;

    println!("Result: {:?}", result);

    assert!(result.is_ok());
  }
}
