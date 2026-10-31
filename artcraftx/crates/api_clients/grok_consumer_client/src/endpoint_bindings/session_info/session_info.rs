//! `GET /api/auth/session` — who the cookies authenticate as. Captured in
//! `external/requests/sites/grok.com/2026-08-23-imagine/09_session_info.txt`.

use crate::client::browser_user_agents::FIREFOX_143_MAC_USER_AGENT;
use crate::client::grok_domain::GrokDomain;
use crate::credentials::grok_cookies::GrokCookies;
use crate::credentials::grok_request_headers::GrokRequestHeaders;
use crate::error::categorize_grok_http_error::categorize_grok_http_error;
use crate::error::grok_error::GrokError;
use crate::error::grok_generic_api_error::GrokGenericApiError;
use crate::utils::create_firefox_client::create_firefox_client;
use log::error;
use serde::Deserialize;
use std::time::Duration;
use wreq::StatusCode;
use wreq::header::{ACCEPT, ACCEPT_LANGUAGE, COOKIE, HeaderMap, REFERER, USER_AGENT};

const SESSION_INFO_PATH: &str = "/api/auth/session";

/// The session's `status` value when the cookies are valid.
const STATUS_AUTHENTICATED: &str = "authenticated";

/// Request arguments. The endpoint currently takes none; this exists so
/// arguments slot in without changing call sites.
#[derive(Clone, Debug, Default)]
pub struct SessionInfoRequest {}

pub struct SessionInfoArgs<'a> {
  pub request: SessionInfoRequest,
  pub credentials: &'a GrokCookies,
  pub domain_override: Option<&'a GrokDomain>,
  /// Optional captured statsig/tracing headers (see [`GrokRequestHeaders`]).
  pub request_headers: Option<&'a GrokRequestHeaders>,
  pub request_timeout: Option<Duration>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SessionInfoResponse {
  /// e.g. "authenticated"
  pub status: String,
  pub session: Option<SessionInfo>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
  pub user_id: String,
  pub email: Option<String>,
  pub email_confirmed: Option<bool>,
  pub given_name: Option<String>,
  pub family_name: Option<String>,
  /// Bucket path of the profile picture, if any.
  pub profile_image: Option<String>,
  pub session_id: Option<String>,
  /// Epoch milliseconds of account creation.
  pub create_time: Option<i64>,
}

impl SessionInfoResponse {
  pub fn is_authenticated(&self) -> bool {
    self.status == STATUS_AUTHENTICATED && self.session.is_some()
  }
}

/// The raw HTTP response, before status interpretation or JSON parsing.
/// Exposed by [`SessionInfoArgs::fetch_raw`] so callers (e.g. diagnostic
/// tests) can inspect the status, headers, and body when a request goes wrong.
pub struct RawSessionInfoResponse {
  pub status: StatusCode,
  pub headers: HeaderMap,
  pub body: String,
}

impl SessionInfoArgs<'_> {
  pub async fn send(&self) -> Result<SessionInfoResponse, GrokError> {
    let raw = self.fetch_raw().await?;

    if !raw.status.is_success() {
      error!("session_info returned an error (code {}): {:?}", raw.status.as_u16(), raw.body);
      return Err(categorize_grok_http_error(raw.status, Some(&raw.body)));
    }

    parse_response(&raw.body)
  }

  /// Send the request and return the raw response parts without interpreting
  /// the status or parsing the body.
  pub async fn fetch_raw(&self) -> Result<RawSessionInfoResponse, GrokError> {
    let domain = self.domain_override.unwrap_or(&GrokDomain::DefaultDomain);
    let client = create_firefox_client()?;

    let mut request_builder = client.get(request_url(domain))
        .header(USER_AGENT, FIREFOX_143_MAC_USER_AGENT)
        .header(ACCEPT, "*/*")
        .header(ACCEPT_LANGUAGE, "en-US,en;q=0.5")
        .header(REFERER, "https://grok.com/imagine/saved")
        .header(COOKIE, self.credentials.to_string())
        .header("sec-fetch-dest", "empty")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-site", "same-origin");

    if let Some(headers) = self.request_headers {
      request_builder = headers.apply(request_builder);
    }

    if let Some(timeout) = self.request_timeout {
      request_builder = request_builder.timeout(timeout);
    }

    let http_request = request_builder
        .build()
        .map_err(|err| {
          error!("Error building session_info request: {:?}", err);
          GrokGenericApiError::WreqError(err)
        })?;

    let response = client.execute(http_request)
        .await
        .map_err(|err| {
          error!("Error sending session_info request: {:?}", err);
          GrokGenericApiError::WreqError(err)
        })?;

    let status = response.status();
    let headers = response.headers().clone();

    let body = response.text()
        .await
        .map_err(|err| {
          error!("Error reading session_info response body: {:?}", err);
          GrokGenericApiError::WreqError(err)
        })?;

    Ok(RawSessionInfoResponse { status, headers, body })
  }
}

fn request_url(domain: &GrokDomain) -> String {
  format!("{}{}", domain.get_domain(), SESSION_INFO_PATH)
}

fn parse_response(body: &str) -> Result<SessionInfoResponse, GrokError> {
  serde_json::from_str(body)
      .map_err(|err| {
        GrokGenericApiError::SerdeResponseParseErrorWithBody(err, body.to_string()).into()
      })
}

#[cfg(test)]
mod tests {
  use super::*;

  // Cargo runs tests with the crate root as the working directory.
  fn load_response(file_name: &str) -> String {
    std::fs::read_to_string(format!("test_data/endpoint_responses/{file_name}")).unwrap()
  }

  mod wire_format_tests {
    use super::*;

    #[test]
    fn url_uses_the_default_domain() {
      assert_eq!(
        request_url(&GrokDomain::DefaultDomain),
        "https://grok.com/api/auth/session",
      );
    }

    #[test]
    fn url_respects_a_domain_override() {
      let domain = GrokDomain::Custom("http://localhost:8080".to_string());
      assert_eq!(request_url(&domain), "http://localhost:8080/api/auth/session");
    }
  }

  mod response_parsing_tests {
    use super::*;

    #[test]
    fn parses_real_session_response() {
      let response = parse_response(&load_response("session_info.json")).unwrap();

      assert_eq!(response.status, "authenticated");
      assert!(response.is_authenticated());

      let session = response.session.unwrap();
      assert_eq!(session.user_id, "00000000-0000-4000-8000-000000000000");
      assert_eq!(session.email.as_deref(), Some("user@example.com"));
      assert_eq!(session.email_confirmed, Some(true));
      assert_eq!(session.given_name.as_deref(), Some("Test"));
      assert_eq!(session.family_name.as_deref(), Some("User"));
      assert_eq!(
        session.session_id.as_deref(),
        Some("88888888-8888-4888-8888-888888888888"),
      );
      assert_eq!(session.create_time, Some(1787514616739));
    }

    #[test]
    fn missing_required_status_is_an_error() {
      // `status` is required, so an object without it fails to parse.
      let result = parse_response("{}");
      assert!(matches!(
        result,
        Err(GrokError::ApiGeneric(GrokGenericApiError::SerdeResponseParseErrorWithBody(_, _))),
      ));
    }
  }

  mod real_wire_tests {
    use super::*;
    use crate::test_utils::grok_test_secrets::load_grok_test_secrets;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    use errors::AnyhowResult;
    use log::LevelFilter;

    // Reaching `Ok` here means `send` saw a 2xx (it returns `Err` otherwise),
    // so a successful parse is the 200 assertion.
    #[tokio::test]
    #[ignore] // Hits the real website; requires external/credentials/grok.
    async fn fetch_session_info() -> AnyhowResult<()> {
      setup_test_logging(LevelFilter::Info);
      let secrets = load_grok_test_secrets()?;

      let args = SessionInfoArgs {
        request: SessionInfoRequest::default(),
        credentials: &secrets.cookies,
        domain_override: None,
        request_headers: Some(&secrets.headers),
        request_timeout: None,
      };

      let response = args.send().await?;
      println!("[test] session_info: {:?}", response);

      // The cookies should authenticate to *some* account. (Assert the shape,
      // not the specific user id / email.)
      assert_eq!(response.status, "authenticated");
      assert!(response.is_authenticated());
      let session = response.session.as_ref().expect("session present");
      assert!(!session.user_id.is_empty());
      Ok(())
    }

    // Same request and assertions as above, but WITHOUT the captured
    // statsig/tracing headers — just the cookies. Tells us whether those
    // headers are actually required. On any failure (non-2xx, parse error, or
    // a failed assertion) it dumps the response status, headers, and body.
    #[tokio::test]
    #[ignore] // Hits the real website; requires external/credentials/grok.
    async fn fetch_session_info_cookies_only() -> AnyhowResult<()> {
      setup_test_logging(LevelFilter::Info);
      let secrets = load_grok_test_secrets()?;

      let args = SessionInfoArgs {
        request: SessionInfoRequest::default(),
        credentials: &secrets.cookies,
        domain_override: None,
        request_headers: None, // cookies only — no statsig / tracing headers
        request_timeout: None,
      };

      let raw = args.fetch_raw().await?;

      let dump = || {
        println!("[test] status: {}", raw.status);
        println!("[test] response headers: {:#?}", raw.headers);
        println!("[test] response body: {}", raw.body);
      };

      if !raw.status.is_success() {
        dump();
        panic!("session_info (cookies only) expected 2xx, got {}", raw.status);
      }

      let response: SessionInfoResponse = match serde_json::from_str(&raw.body) {
        Ok(response) => response,
        Err(err) => {
          dump();
          panic!("session_info (cookies only) failed to parse body: {err}");
        }
      };
      println!("[test] session_info (cookies only): {:?}", response);

      // Same assertions as fetch_session_info, but dump diagnostics on failure.
      let assertions_pass = response.status == "authenticated"
          && response.is_authenticated()
          && response.session.as_ref().is_some_and(|session| !session.user_id.is_empty());

      if !assertions_pass {
        dump();
        panic!("session_info (cookies only) assertions failed");
      }

      Ok(())
    }
  }
}
