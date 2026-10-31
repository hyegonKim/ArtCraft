//! `GET /rest/assets` — list the user's generated imagine assets (newest
//! first). Captured in
//! `external/requests/sites/grok.com/2026-08-23-imagine/11_list_assets.txt`.

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
use std::collections::HashMap;
use std::time::Duration;
use wreq::StatusCode;
use wreq::header::{ACCEPT, ACCEPT_LANGUAGE, COOKIE, HeaderMap, REFERER, USER_AGENT};

const LIST_ASSETS_PATH: &str = "/rest/assets";

/// Page size used when the request doesn't specify one (the website's own).
const DEFAULT_PAGE_SIZE: u32 = 40;

/// Fixed query parameters observed in the capture.
const ORDER_BY: &str = "ORDER_BY_CREATE_TIME";
const WORKSPACE_KIND: &str = "WORKSPACE_KIND_IMAGINE_ALL";

#[derive(Clone, Debug, Default)]
pub struct ListAssetsRequest {
  /// Number of assets per page. Defaults to 40 (the website's own).
  pub page_size: Option<u32>,
}

pub struct ListAssetsArgs<'a> {
  pub request: ListAssetsRequest,
  pub credentials: &'a GrokCookies,
  pub domain_override: Option<&'a GrokDomain>,
  /// Optional captured statsig/tracing headers (see [`GrokRequestHeaders`]).
  pub request_headers: Option<&'a GrokRequestHeaders>,
  pub request_timeout: Option<Duration>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ListAssetsResponse {
  pub assets: Vec<GrokAsset>,
}

/// One generated (or uploaded) file. Also appears as `latestAssetMetadata`
/// in the imagine conversation listing.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokAsset {
  pub asset_id: String,
  /// e.g. "image/jpeg", "video/mp4"
  pub mime_type: Option<String>,
  /// e.g. "image.jpg", "generated_video.mp4"
  pub name: Option<String>,
  pub size_bytes: Option<u64>,
  /// RFC 3339, e.g. "2026-08-23T20:01:10.223Z"
  pub create_time: Option<String>,
  /// Bucket path of the primary file,
  /// e.g. `users/{user_id}/generated/{asset_id}/image.jpg`.
  pub key: Option<String>,
  /// Extra file paths and flags, e.g. "preview-image", "original-image",
  /// "thumbhash", "moderated".
  #[serde(default)]
  pub aux_keys: HashMap<String, String>,
  pub response_id: Option<String>,
  pub is_deleted: Option<bool>,
  pub is_latest: Option<bool>,
  pub source_conversation_id: Option<String>,
  pub width: Option<u32>,
  pub height: Option<u32>,
  /// The generation request that produced the asset. Shape varies by mode
  /// (`textToImage`, `imageToVideo`, ...), so it stays untyped for now.
  pub media_gen_input: Option<serde_json::Value>,
}

/// The raw HTTP response, before status interpretation or JSON parsing.
/// Exposed by [`ListAssetsArgs::fetch_raw`] so callers (e.g. diagnostic tests)
/// can inspect the status, headers, and body when a request goes wrong.
pub struct RawListAssetsResponse {
  pub status: StatusCode,
  pub headers: HeaderMap,
  pub body: String,
}

impl ListAssetsArgs<'_> {
  pub async fn send(&self) -> Result<ListAssetsResponse, GrokError> {
    let raw = self.fetch_raw().await?;

    if !raw.status.is_success() {
      error!("list_assets returned an error (code {}): {:?}", raw.status.as_u16(), raw.body);
      return Err(categorize_grok_http_error(raw.status, Some(&raw.body)));
    }

    parse_response(&raw.body)
  }

  /// Send the request and return the raw response parts without interpreting
  /// the status or parsing the body.
  pub async fn fetch_raw(&self) -> Result<RawListAssetsResponse, GrokError> {
    let domain = self.domain_override.unwrap_or(&GrokDomain::DefaultDomain);
    let client = create_firefox_client()?;

    let mut request_builder = client.get(request_url(domain, &self.request))
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
          error!("Error building list_assets request: {:?}", err);
          GrokGenericApiError::WreqError(err)
        })?;

    let response = client.execute(http_request)
        .await
        .map_err(|err| {
          error!("Error sending list_assets request: {:?}", err);
          GrokGenericApiError::WreqError(err)
        })?;

    let status = response.status();
    let headers = response.headers().clone();

    let body = response.text()
        .await
        .map_err(|err| {
          error!("Error reading list_assets response body: {:?}", err);
          GrokGenericApiError::WreqError(err)
        })?;

    Ok(RawListAssetsResponse { status, headers, body })
  }
}

fn request_url(domain: &GrokDomain, request: &ListAssetsRequest) -> String {
  let page_size = request.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
  format!(
    "{}{}?pageSize={}&orderBy={}&workspaceKind={}",
    domain.get_domain(), LIST_ASSETS_PATH, page_size, ORDER_BY, WORKSPACE_KIND,
  )
}

fn parse_response(body: &str) -> Result<ListAssetsResponse, GrokError> {
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
    fn url_defaults_to_page_size_40() {
      let request = ListAssetsRequest { page_size: None };
      assert_eq!(
        request_url(&GrokDomain::DefaultDomain, &request),
        "https://grok.com/rest/assets?pageSize=40&orderBy=ORDER_BY_CREATE_TIME&workspaceKind=WORKSPACE_KIND_IMAGINE_ALL",
      );
    }

    #[test]
    fn url_respects_page_size_and_domain_overrides() {
      let request = ListAssetsRequest { page_size: Some(7) };
      let domain = GrokDomain::Custom("http://localhost:8080".to_string());
      assert_eq!(
        request_url(&domain, &request),
        "http://localhost:8080/rest/assets?pageSize=7&orderBy=ORDER_BY_CREATE_TIME&workspaceKind=WORKSPACE_KIND_IMAGINE_ALL",
      );
    }
  }

  mod response_parsing_tests {
    use super::*;

    #[test]
    fn parses_real_assets_response() {
      let response = parse_response(&load_response("list_assets.json")).unwrap();
      assert_eq!(response.assets.len(), 2);

      let video = &response.assets[0];
      assert_eq!(video.asset_id, "98db6014-be3a-4c92-9aec-91c80123ccac");
      assert_eq!(video.mime_type.as_deref(), Some("video/mp4"));
      assert_eq!(video.name.as_deref(), Some("generated_video.mp4"));
      assert_eq!(video.size_bytes, Some(5800655));
      assert_eq!(video.width, Some(736));
      assert_eq!(video.height, Some(400));
      assert_eq!(video.is_deleted, Some(false));
      assert_eq!(
        video.aux_keys.get("preview-image").map(String::as_str),
        Some("users/00000000-0000-4000-8000-000000000000/generated/98db6014-be3a-4c92-9aec-91c80123ccac/preview_image.jpg"),
      );
      let video_input = video.media_gen_input.as_ref().unwrap();
      assert_eq!(video_input["imageToVideo"]["duration"], 6);

      let image = &response.assets[1];
      assert_eq!(image.asset_id, "8d0b3727-f040-4b33-abf0-16e7d4364e6c");
      assert_eq!(image.mime_type.as_deref(), Some("image/jpeg"));
      assert_eq!(
        image.source_conversation_id.as_deref(),
        Some("e851b4f1-0605-4a41-b624-d629c2c1638d"),
      );
      let image_input = image.media_gen_input.as_ref().unwrap();
      assert_eq!(
        image_input["textToImage"]["prompt"],
        "A pirate ship in the middle of the forest",
      );
    }

    #[test]
    fn missing_required_assets_is_an_error() {
      // `assets` is required, so an object without it fails to parse.
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

    #[tokio::test]
    #[ignore] // Hits the real website; requires external/credentials/grok.
    async fn fetch_assets() -> AnyhowResult<()> {
      setup_test_logging(LevelFilter::Info);
      let secrets = load_grok_test_secrets()?;

      let page_size: u32 = 5;
      let args = ListAssetsArgs {
        request: ListAssetsRequest { page_size: Some(page_size) },
        credentials: &secrets.cookies,
        domain_override: None,
        request_headers: Some(&secrets.headers),
        request_timeout: None,
      };

      // Fetch raw so we can print the whole body and spot fields the typed
      // response might be dropping.
      let raw = args.fetch_raw().await?;
      println!("[test] full response body:\n{}", pretty_json(&raw.body));
      assert!(raw.status.is_success(), "expected 2xx, got {}", raw.status);

      let response: ListAssetsResponse = serde_json::from_str(&raw.body)?;
      for asset in &response.assets {
        println!("[test] asset: {} ({:?})", asset.asset_id, asset.mime_type);
      }

      assert_list_assets(&response, page_size);
      Ok(())
    }

    // Same request and assertions as above, but WITHOUT the captured
    // statsig/tracing headers — just the cookies. Tells us whether those
    // headers are actually required. On any failure (non-2xx, parse error, or
    // a failed assertion) it dumps the response status, headers, and body.
    #[tokio::test]
    #[ignore] // Hits the real website; requires external/credentials/grok.
    async fn fetch_assets_cookies_only() -> AnyhowResult<()> {
      setup_test_logging(LevelFilter::Info);
      let secrets = load_grok_test_secrets()?;

      let page_size = 5;
      let args = ListAssetsArgs {
        request: ListAssetsRequest { page_size: Some(page_size) },
        credentials: &secrets.cookies,
        domain_override: None,
        request_headers: None, // cookies only — no statsig / tracing headers
        request_timeout: None,
      };

      let raw = args.fetch_raw().await?;
      println!("[test] full response body:\n{}", pretty_json(&raw.body));

      let dump = || {
        println!("[test] status: {}", raw.status);
        println!("[test] response headers: {:#?}", raw.headers);
        println!("[test] response body: {}", raw.body);
      };

      if !raw.status.is_success() {
        dump();
        panic!("list_assets (cookies only) expected 2xx, got {}", raw.status);
      }

      let response: ListAssetsResponse = match serde_json::from_str(&raw.body) {
        Ok(response) => response,
        Err(err) => {
          dump();
          panic!("list_assets (cookies only) failed to parse body: {err}");
        }
      };
      for asset in &response.assets {
        println!("[test] asset: {} ({:?})", asset.asset_id, asset.mime_type);
      }

      if !list_assets_assertions_pass(&response, page_size) {
        dump();
        panic!("list_assets (cookies only) assertions failed");
      }
      Ok(())
    }

    /// Pretty-print a JSON body for inspection, falling back to the raw string.
    fn pretty_json(body: &str) -> String {
      serde_json::from_str::<serde_json::Value>(body)
          .and_then(|value| serde_json::to_string_pretty(&value))
          .unwrap_or_else(|_| body.to_string())
    }

    // The account under test has generated media, so expect a non-empty page
    // whose entries carry the structural fields. (No PII asserted.)
    fn list_assets_assertions_pass(response: &ListAssetsResponse, page_size: u32) -> bool {
      !response.assets.is_empty()
          && response.assets.len() <= page_size as usize
          && response.assets.iter().all(|asset| {
            !asset.asset_id.is_empty() && asset.mime_type.is_some()
          })
    }

    fn assert_list_assets(response: &ListAssetsResponse, page_size: u32) {
      assert!(!response.assets.is_empty(), "expected at least one asset");
      assert!(list_assets_assertions_pass(response, page_size), "asset assertions failed");
    }
  }
}
