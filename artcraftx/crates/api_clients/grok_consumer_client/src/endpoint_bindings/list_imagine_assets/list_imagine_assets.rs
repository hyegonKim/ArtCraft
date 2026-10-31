//! `GET /rest/app-chat/conversations?kind=CONVERSATION_KIND_IMAGINE` — list
//! the user's imagine conversations, each carrying its latest generated
//! asset. Captured in
//! `external/requests/sites/grok.com/2026-08-23-imagine/10_list_imagine_conversations.txt`.

use crate::client::browser_user_agents::FIREFOX_143_MAC_USER_AGENT;
use crate::client::grok_domain::GrokDomain;
use crate::credentials::grok_cookies::GrokCookies;
use crate::credentials::grok_request_headers::GrokRequestHeaders;
use crate::endpoint_bindings::list_assets::list_assets::GrokAsset;
use crate::error::categorize_grok_http_error::categorize_grok_http_error;
use crate::error::grok_error::GrokError;
use crate::error::grok_generic_api_error::GrokGenericApiError;
use crate::utils::create_firefox_client::create_firefox_client;
use log::error;
use serde::Deserialize;
use std::time::Duration;
use wreq::StatusCode;
use wreq::header::{ACCEPT, ACCEPT_LANGUAGE, COOKIE, HeaderMap, REFERER, USER_AGENT};

const LIST_IMAGINE_ASSETS_PATH: &str = "/rest/app-chat/conversations";

/// Page size used when the request doesn't specify one (the website's own).
const DEFAULT_PAGE_SIZE: u32 = 40;

/// Fixed query parameter observed in the capture.
const CONVERSATION_KIND: &str = "CONVERSATION_KIND_IMAGINE";

#[derive(Clone, Debug, Default)]
pub struct ListImagineAssetsRequest {
  /// Number of conversations per page. Defaults to 40 (the website's own).
  pub page_size: Option<u32>,
}

pub struct ListImagineAssetsArgs<'a> {
  pub request: ListImagineAssetsRequest,
  pub credentials: &'a GrokCookies,
  pub domain_override: Option<&'a GrokDomain>,
  /// Optional captured statsig/tracing headers (see [`GrokRequestHeaders`]).
  pub request_headers: Option<&'a GrokRequestHeaders>,
  pub request_timeout: Option<Duration>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ListImagineAssetsResponse {
  pub conversations: Vec<ImagineConversation>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagineConversation {
  pub conversation_id: String,
  /// Often empty for imagine conversations.
  pub title: Option<String>,
  pub starred: Option<bool>,
  /// RFC 3339, e.g. "2026-08-23T20:00:41.151254Z"
  pub create_time: Option<String>,
  pub modify_time: Option<String>,
  /// e.g. "CONVERSATION_KIND_IMAGINE"
  pub kind: Option<String>,
  /// The conversation's most recent generated asset.
  pub latest_asset_metadata: Option<GrokAsset>,
}

/// The raw HTTP response, before status interpretation or JSON parsing.
/// Exposed by [`ListImagineAssetsArgs::fetch_raw`] so callers (e.g. diagnostic
/// tests) can inspect the status, headers, and body when a request goes wrong.
pub struct RawListImagineAssetsResponse {
  pub status: StatusCode,
  pub headers: HeaderMap,
  pub body: String,
}

impl ListImagineAssetsArgs<'_> {
  pub async fn send(&self) -> Result<ListImagineAssetsResponse, GrokError> {
    let raw = self.fetch_raw().await?;

    if !raw.status.is_success() {
      error!("list_imagine_assets returned an error (code {}): {:?}", raw.status.as_u16(), raw.body);
      return Err(categorize_grok_http_error(raw.status, Some(&raw.body)));
    }

    parse_response(&raw.body)
  }

  /// Send the request and return the raw response parts without interpreting
  /// the status or parsing the body.
  pub async fn fetch_raw(&self) -> Result<RawListImagineAssetsResponse, GrokError> {
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
          error!("Error building list_imagine_assets request: {:?}", err);
          GrokGenericApiError::WreqError(err)
        })?;

    let response = client.execute(http_request)
        .await
        .map_err(|err| {
          error!("Error sending list_imagine_assets request: {:?}", err);
          GrokGenericApiError::WreqError(err)
        })?;

    let status = response.status();
    let headers = response.headers().clone();

    let body = response.text()
        .await
        .map_err(|err| {
          error!("Error reading list_imagine_assets response body: {:?}", err);
          GrokGenericApiError::WreqError(err)
        })?;

    Ok(RawListImagineAssetsResponse { status, headers, body })
  }
}

fn request_url(domain: &GrokDomain, request: &ListImagineAssetsRequest) -> String {
  let page_size = request.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
  format!(
    "{}{}?pageSize={}&kind={}",
    domain.get_domain(), LIST_IMAGINE_ASSETS_PATH, page_size, CONVERSATION_KIND,
  )
}

fn parse_response(body: &str) -> Result<ListImagineAssetsResponse, GrokError> {
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
      let request = ListImagineAssetsRequest { page_size: None };
      assert_eq!(
        request_url(&GrokDomain::DefaultDomain, &request),
        "https://grok.com/rest/app-chat/conversations?pageSize=40&kind=CONVERSATION_KIND_IMAGINE",
      );
    }

    #[test]
    fn url_respects_page_size_and_domain_overrides() {
      let request = ListImagineAssetsRequest { page_size: Some(12) };
      let domain = GrokDomain::Custom("http://localhost:8080".to_string());
      assert_eq!(
        request_url(&domain, &request),
        "http://localhost:8080/rest/app-chat/conversations?pageSize=12&kind=CONVERSATION_KIND_IMAGINE",
      );
    }
  }

  mod response_parsing_tests {
    use super::*;

    #[test]
    fn parses_real_conversations_response() {
      let response = parse_response(&load_response("list_imagine_assets.json")).unwrap();
      assert_eq!(response.conversations.len(), 2);

      let video_conversation = &response.conversations[0];
      assert_eq!(
        video_conversation.conversation_id,
        "9db05fac-a4df-4a42-aa81-4dee6d37f4a8",
      );
      assert_eq!(
        video_conversation.title.as_deref(),
        Some("Video Generation with AI"),
      );
      assert_eq!(video_conversation.starred, Some(false));
      assert_eq!(
        video_conversation.kind.as_deref(),
        Some("CONVERSATION_KIND_IMAGINE"),
      );

      let video_asset = video_conversation.latest_asset_metadata.as_ref().unwrap();
      assert_eq!(video_asset.asset_id, "98db6014-be3a-4c92-9aec-91c80123ccac");
      assert_eq!(video_asset.mime_type.as_deref(), Some("video/mp4"));

      let image_conversation = &response.conversations[1];
      assert_eq!(image_conversation.title.as_deref(), Some(""));
      let image_asset = image_conversation.latest_asset_metadata.as_ref().unwrap();
      assert_eq!(image_asset.asset_id, "8d0b3727-f040-4b33-abf0-16e7d4364e6c");
      let image_input = image_asset.media_gen_input.as_ref().unwrap();
      assert_eq!(
        image_input["textToImage"]["prompt"],
        "A pirate ship in the middle of the forest",
      );
    }

    #[test]
    fn missing_required_conversations_is_an_error() {
      // `conversations` is required, so an object without it fails to parse.
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
    async fn fetch_imagine_conversations() -> AnyhowResult<()> {
      setup_test_logging(LevelFilter::Info);
      let secrets = load_grok_test_secrets()?;

      let page_size: u32 = 5;
      let args = ListImagineAssetsArgs {
        request: ListImagineAssetsRequest { page_size: Some(page_size) },
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

      let response: ListImagineAssetsResponse = serde_json::from_str(&raw.body)?;
      print_conversations(&response);

      assert_list_imagine_assets(&response, page_size);
      Ok(())
    }

    // Same request and assertions as above, but WITHOUT the captured
    // statsig/tracing headers — just the cookies. Tells us whether those
    // headers are actually required. On any failure (non-2xx, parse error, or
    // a failed assertion) it dumps the response status, headers, and body.
    #[tokio::test]
    #[ignore] // Hits the real website; requires external/credentials/grok.
    async fn fetch_imagine_conversations_cookies_only() -> AnyhowResult<()> {
      setup_test_logging(LevelFilter::Info);
      let secrets = load_grok_test_secrets()?;

      let page_size = 5;
      let args = ListImagineAssetsArgs {
        request: ListImagineAssetsRequest { page_size: Some(page_size) },
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
        panic!("list_imagine_assets (cookies only) expected 2xx, got {}", raw.status);
      }

      let response: ListImagineAssetsResponse = match serde_json::from_str(&raw.body) {
        Ok(response) => response,
        Err(err) => {
          dump();
          panic!("list_imagine_assets (cookies only) failed to parse body: {err}");
        }
      };
      print_conversations(&response);

      if !list_imagine_assets_assertions_pass(&response, page_size) {
        dump();
        panic!("list_imagine_assets (cookies only) assertions failed");
      }
      Ok(())
    }

    /// Pretty-print a JSON body for inspection, falling back to the raw string.
    fn pretty_json(body: &str) -> String {
      serde_json::from_str::<serde_json::Value>(body)
          .and_then(|value| serde_json::to_string_pretty(&value))
          .unwrap_or_else(|_| body.to_string())
    }

    fn print_conversations(response: &ListImagineAssetsResponse) {
      for conversation in &response.conversations {
        println!(
          "[test] conversation: {} (latest asset: {:?})",
          conversation.conversation_id,
          conversation.latest_asset_metadata.as_ref().map(|a| &a.asset_id),
        );
      }
    }

    // The account under test has imagine conversations; expect a non-empty
    // page of the imagine kind. (No PII asserted.)
    fn list_imagine_assets_assertions_pass(
      response: &ListImagineAssetsResponse,
      page_size: u32,
    ) -> bool {
      !response.conversations.is_empty()
          && response.conversations.len() <= page_size as usize
          && response.conversations.iter().all(|conversation| {
            !conversation.conversation_id.is_empty()
                && conversation.kind.as_deref().is_none_or(|kind| kind == "CONVERSATION_KIND_IMAGINE")
          })
    }

    fn assert_list_imagine_assets(response: &ListImagineAssetsResponse, page_size: u32) {
      assert!(!response.conversations.is_empty(), "expected a conversation");
      assert!(
        list_imagine_assets_assertions_pass(response, page_size),
        "conversation assertions failed",
      );
    }
  }
}
