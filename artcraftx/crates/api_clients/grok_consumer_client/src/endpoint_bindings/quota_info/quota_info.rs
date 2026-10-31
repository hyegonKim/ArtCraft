//! `POST /rest/media/imagine/quota_info` — remaining imagine generation
//! quota per media kind. Captured in
//! `external/requests/sites/grok.com/2026-08-23-imagine/12_quota_info.txt`.

use crate::client::browser_user_agents::FIREFOX_143_MAC_USER_AGENT;
use crate::client::grok_domain::GrokDomain;
use crate::credentials::grok_cookies::GrokCookies;
use crate::credentials::grok_request_headers::GrokRequestHeaders;
use crate::error::categorize_grok_http_error::categorize_grok_http_error;
use crate::error::grok_error::GrokError;
use crate::error::grok_generic_api_error::GrokGenericApiError;
use crate::utils::create_firefox_client::create_firefox_client;
use log::error;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use wreq::StatusCode;
use wreq::header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, COOKIE, HeaderMap, ORIGIN, REFERER, USER_AGENT};

const QUOTA_INFO_PATH: &str = "/rest/media/imagine/quota_info";

/// Request arguments. The endpoint currently takes none (the body is `{}`);
/// this exists so arguments slot in without changing call sites.
#[derive(Clone, Debug, Default)]
pub struct QuotaInfoRequest {}

pub struct QuotaInfoArgs<'a> {
  pub request: QuotaInfoRequest,
  pub credentials: &'a GrokCookies,
  pub domain_override: Option<&'a GrokDomain>,
  /// Optional captured statsig/tracing headers (see [`GrokRequestHeaders`]).
  pub request_headers: Option<&'a GrokRequestHeaders>,
  pub request_timeout: Option<Duration>,
}

/// Quota per media kind. Kinds the account has no quota line for are `null`.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaInfoResponse {
  pub image: Option<QuotaInfoBucket>,
  pub image_pro: Option<QuotaInfoBucket>,
  pub image_edit: Option<QuotaInfoBucket>,
  pub video: Option<QuotaInfoBucket>,
  #[serde(rename = "video720p")]
  pub video_720p: Option<QuotaInfoBucket>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaInfoBucket {
  pub available: bool,
  pub remaining_queries: u64,
  /// The rolling quota window, e.g. 86400 (one day).
  pub window_size_seconds: u64,
  /// RFC 3339 timestamp; present when the bucket is exhausted.
  pub next_available_at: Option<String>,
}

/// Wire body — always the empty object.
#[derive(Serialize)]
struct QuotaInfoRawRequest {}

/// The raw HTTP response, before status interpretation or JSON parsing.
/// Exposed by [`QuotaInfoArgs::fetch_raw`] so callers (e.g. diagnostic tests)
/// can inspect the status, headers, and body when a request goes wrong.
pub struct RawQuotaInfoResponse {
  pub status: StatusCode,
  pub headers: HeaderMap,
  pub body: String,
}

impl QuotaInfoArgs<'_> {
  pub async fn send(&self) -> Result<QuotaInfoResponse, GrokError> {
    let raw = self.fetch_raw().await?;

    if !raw.status.is_success() {
      error!("quota_info returned an error (code {}): {:?}", raw.status.as_u16(), raw.body);
      return Err(categorize_grok_http_error(raw.status, Some(&raw.body)));
    }

    parse_response(&raw.body)
  }

  /// Send the request and return the raw response parts without interpreting
  /// the status or parsing the body.
  pub async fn fetch_raw(&self) -> Result<RawQuotaInfoResponse, GrokError> {
    let domain = self.domain_override.unwrap_or(&GrokDomain::DefaultDomain);
    let client = create_firefox_client()?;

    let mut request_builder = client.post(request_url(domain))
        .header(USER_AGENT, FIREFOX_143_MAC_USER_AGENT)
        .header(ACCEPT, "*/*")
        .header(ACCEPT_LANGUAGE, "en-US,en;q=0.5")
        .header(CONTENT_TYPE, "application/json")
        .header(ORIGIN, "https://grok.com")
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

    let http_request = request_builder.json(&QuotaInfoRawRequest {})
        .build()
        .map_err(|err| {
          error!("Error building quota_info request: {:?}", err);
          GrokGenericApiError::WreqError(err)
        })?;

    let response = client.execute(http_request)
        .await
        .map_err(|err| {
          error!("Error sending quota_info request: {:?}", err);
          GrokGenericApiError::WreqError(err)
        })?;

    let status = response.status();
    let headers = response.headers().clone();

    let body = response.text()
        .await
        .map_err(|err| {
          error!("Error reading quota_info response body: {:?}", err);
          GrokGenericApiError::WreqError(err)
        })?;

    Ok(RawQuotaInfoResponse { status, headers, body })
  }
}

fn request_url(domain: &GrokDomain) -> String {
  format!("{}{}", domain.get_domain(), QUOTA_INFO_PATH)
}

fn parse_response(body: &str) -> Result<QuotaInfoResponse, GrokError> {
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
        "https://grok.com/rest/media/imagine/quota_info",
      );
    }

    #[test]
    fn url_respects_a_domain_override() {
      let domain = GrokDomain::Custom("http://localhost:8080".to_string());
      assert_eq!(
        request_url(&domain),
        "http://localhost:8080/rest/media/imagine/quota_info",
      );
    }

    #[test]
    fn body_is_the_empty_object() {
      let body = serde_json::to_string(&QuotaInfoRawRequest {}).unwrap();
      assert_eq!(body, "{}");
    }
  }

  mod response_parsing_tests {
    use super::*;

    #[test]
    fn parses_real_quota_response() {
      let response = parse_response(&load_response("quota_info.json")).unwrap();

      assert!(response.image.is_none());
      assert!(response.image_edit.is_none());
      assert!(response.video.is_none());

      let image_pro = response.image_pro.unwrap();
      assert!(image_pro.available);
      assert_eq!(image_pro.remaining_queries, 4);
      assert_eq!(image_pro.window_size_seconds, 86400);
      assert!(image_pro.next_available_at.is_none());

      let video_720p = response.video_720p.unwrap();
      assert!(video_720p.available);
      assert_eq!(video_720p.remaining_queries, 0);
      assert_eq!(
        video_720p.next_available_at.as_deref(),
        Some("2026-08-24T21:00:41.230702760Z"),
      );
    }

    #[test]
    fn malformed_json_is_an_error() {
      let result = parse_response("{ this is not json");
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
    async fn fetch_quota_info() -> AnyhowResult<()> {
      setup_test_logging(LevelFilter::Info);
      let secrets = load_grok_test_secrets()?;

      let args = QuotaInfoArgs {
        request: QuotaInfoRequest::default(),
        credentials: &secrets.cookies,
        domain_override: None,
        request_headers: Some(&secrets.headers),
        request_timeout: None,
      };

      let response = args.send().await?;
      println!("[test] quota_info: {:?}", response);

      assert_quota_info(&response);
      Ok(())
    }

    // Same request and assertions as above, but WITHOUT the captured
    // statsig/tracing headers — just the cookies. Tells us whether those
    // headers are actually required. On any failure (non-2xx, parse error, or
    // a failed assertion) it dumps the response status, headers, and body.
    #[tokio::test]
    #[ignore] // Hits the real website; requires external/credentials/grok.
    async fn fetch_quota_info_cookies_only() -> AnyhowResult<()> {
      setup_test_logging(LevelFilter::Info);
      let secrets = load_grok_test_secrets()?;

      let args = QuotaInfoArgs {
        request: QuotaInfoRequest::default(),
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
        panic!("quota_info (cookies only) expected 2xx, got {}", raw.status);
      }

      let response: QuotaInfoResponse = match serde_json::from_str(&raw.body) {
        Ok(response) => response,
        Err(err) => {
          dump();
          panic!("quota_info (cookies only) failed to parse body: {err}");
        }
      };
      println!("[test] quota_info (cookies only): {:?}", response);

      if !quota_info_assertions_pass(&response) {
        dump();
        panic!("quota_info (cookies only) assertions failed");
      }
      Ok(())
    }

    // At least one media kind should report a quota bucket, and every present
    // bucket has a positive rolling window. (No PII / account id.)
    fn quota_info_assertions_pass(response: &QuotaInfoResponse) -> bool {
      let buckets = quota_buckets(response);
      buckets.iter().any(|bucket| bucket.is_some())
          && buckets.into_iter().flatten().all(|bucket| bucket.window_size_seconds > 0)
    }

    fn assert_quota_info(response: &QuotaInfoResponse) {
      assert!(
        quota_info_assertions_pass(response),
        "expected at least one quota bucket with a positive window",
      );
    }

    fn quota_buckets(response: &QuotaInfoResponse) -> [&Option<QuotaInfoBucket>; 5] {
      [
        &response.image,
        &response.image_pro,
        &response.image_edit,
        &response.video,
        &response.video_720p,
      ]
    }
  }
}
