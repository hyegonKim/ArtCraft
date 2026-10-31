//! The one HTTP path every endpoint uses: build an emulated-browser client
//! matching the capturing browser, attach the session headers, send, classify
//! the status (bot protection first), and deserialize.

use crate::client::clerk_host::ClerkHost;
use crate::client::higgsfield_browser_profile::higgsfield_browser_profile;
use crate::client::higgsfield_host::{HiggsfieldHost, WEB_ORIGIN};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::credentials::higgsfield_cookies::HiggsfieldCookies;
use crate::error::classify_higgsfield_http_error::{classify_higgsfield_http_response, HttpResponseSignals};
use crate::error::higgsfield_api_error::HiggsfieldApiError;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use log::{debug, info};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::time::Duration;
use wreq::header::HeaderMap;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Uploads carry whole files; give them longer than an API call.
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Copy)]
pub(crate) enum HttpMethod {
  Get,
  Post,
}

impl HttpMethod {
  fn as_str(self) -> &'static str {
    match self {
      Self::Get => "GET",
      Self::Post => "POST",
    }
  }
}

/// How a request proves who it is.
pub(crate) enum RequestCredential<'a> {
  /// The API gateway: `authorization: Bearer <jwt>` (+ optional cookies and
  /// DataDome id).
  Bearer(&'a HiggsfieldAuth),

  /// Clerk's frontend API: just the browser cookies (`__client`).
  Cookies {
    cookies: &'a HiggsfieldCookies,
    /// The capturing browser's UA, for the same reason as on the gateway.
    maybe_user_agent: Option<&'a str>,
  },
}

impl RequestCredential<'_> {
  fn maybe_user_agent(&self) -> Option<&str> {
    match self {
      Self::Bearer(auth) => auth.maybe_user_agent.as_deref(),
      Self::Cookies { maybe_user_agent, .. } => *maybe_user_agent,
    }
  }
}

pub(crate) enum RequestBody<'a, Body: Serialize> {
  None,
  Json(&'a Body),
  Form(&'a Body),
}

/// Send a JSON request to the Higgsfield API gateway and parse a JSON
/// response. `maybe_body` is serialized for POSTs; pass `None::<&()>` for
/// GETs.
pub(crate) async fn send_json_request<Response, Body>(
  method: HttpMethod,
  path: &str,
  auth: &HiggsfieldAuth,
  host: &HiggsfieldHost,
  maybe_body: Option<&Body>,
) -> Result<Response, HiggsfieldError>
where
  Response: DeserializeOwned,
  Body: Serialize,
{
  auth.validate()?;
  let body = match maybe_body {
    Some(body) => RequestBody::Json(body),
    None => RequestBody::None,
  };
  send_request(method, host.url(path), RequestCredential::Bearer(auth), body).await
}

/// Send a request to Clerk's frontend API (cookie-authenticated) and parse a
/// JSON response.
pub(crate) async fn send_clerk_request<Response, Body>(
  method: HttpMethod,
  path: &str,
  cookies: &HiggsfieldCookies,
  maybe_user_agent: Option<&str>,
  host: &ClerkHost,
  body: RequestBody<'_, Body>,
) -> Result<Response, HiggsfieldError>
where
  Response: DeserializeOwned,
  Body: Serialize,
{
  cookies.validate()?;
  send_request(method, host.url(path), RequestCredential::Cookies { cookies, maybe_user_agent }, body).await
}

/// Upload raw bytes to a presigned storage URL (the `upload_url` from the
/// media presign endpoints). No Higgsfield auth is involved — the signature
/// in the URL is the credential — but the browser identity is kept, since
/// the bucket's CORS policy and the presign were both made for the web app.
///
/// Storage answers an empty `200`; any other status is classified like a
/// gateway response so callers get the same error vocabulary.
pub(crate) async fn send_presigned_upload(
  upload_url: &str,
  content_type: &str,
  bytes: Vec<u8>,
  maybe_user_agent: Option<&str>,
) -> Result<(), HiggsfieldError> {
  let browser_profile = higgsfield_browser_profile(maybe_user_agent);

  let client = browser_profile
      .configure_client_builder(wreq::Client::builder())
      .timeout(UPLOAD_TIMEOUT)
      .build()
      .map_err(HiggsfieldClientError::WreqClientBuild)?;

  let byte_count = bytes.len();
  let request = client.put(upload_url)
      .header("accept", "*/*")
      .header("accept-language", "en")
      .header("content-type", content_type)
      .header("origin", WEB_ORIGIN)
      .header("referer", format!("{WEB_ORIGIN}/"))
      .header("sec-fetch-dest", "empty")
      .header("sec-fetch-mode", "cors")
      .header("sec-fetch-site", "cross-site")
      .body(bytes)
      .build()
      .map_err(HiggsfieldClientError::WreqRequestBuild)?;

  // NB: the URL carries the signature; log only its host and path.
  let url_for_log = upload_url.split('?').next().unwrap_or(upload_url);
  info!("Higgsfield media upload: PUT {} ({} bytes, {})", url_for_log, byte_count, content_type);

  let response = client.execute(request)
      .await
      .map_err(HiggsfieldApiError::from_transport_error)?;

  let status = response.status();
  let protection_headers = ProtectionHeaders::from_headers(response.headers());
  let response_body = response.text()
      .await
      .map_err(HiggsfieldApiError::from_transport_error)?;

  info!("Higgsfield media upload response: status={} ({} bytes)", status, response_body.len());
  debug!("Higgsfield media upload response body: {}", response_body);

  classify_higgsfield_http_response(&HttpResponseSignals {
    status_code: status.as_u16(),
    body: &response_body,
    maybe_server_header: protection_headers.server.as_deref(),
    maybe_cf_ray: protection_headers.cf_ray.as_deref(),
    maybe_cf_mitigated: protection_headers.cf_mitigated.as_deref(),
    maybe_x_datadome: protection_headers.x_datadome.as_deref(),
    maybe_x_dd_b: protection_headers.x_dd_b.as_deref(),
    context: url_for_log,
  })
}

async fn send_request<Response, Body>(
  method: HttpMethod,
  url: String,
  credential: RequestCredential<'_>,
  body: RequestBody<'_, Body>,
) -> Result<Response, HiggsfieldError>
where
  Response: DeserializeOwned,
  Body: Serialize,
{
  // Present the browser that earned the cookies (or our pinned default).
  let browser_profile = higgsfield_browser_profile(credential.maybe_user_agent());

  let client = browser_profile
      .configure_client_builder(wreq::Client::builder())
      .timeout(REQUEST_TIMEOUT)
      .build()
      .map_err(HiggsfieldClientError::WreqClientBuild)?;

  let request_builder = match method {
    HttpMethod::Get => client.get(&url),
    HttpMethod::Post => client.post(&url),
  };

  // NB: Browser-identity headers (user-agent, sec-ch-ua*) come from the
  // emulation on the client; only request-context headers are set here.
  let mut request_builder = request_builder
      .header("accept", "*/*")
      .header("accept-language", "en")
      .header("origin", WEB_ORIGIN)
      .header("referer", format!("{WEB_ORIGIN}/"))
      .header("cache-control", "no-cache")
      .header("pragma", "no-cache")
      .header("priority", "u=1, i")
      .header("sec-fetch-dest", "empty")
      .header("sec-fetch-mode", "cors")
      .header("sec-fetch-site", "same-site");

  match &credential {
    RequestCredential::Bearer(auth) => {
      request_builder = request_builder.header("authorization", auth.bearer_header_value());
      if let Some(cookie_header) = auth.maybe_cookie_header.as_deref() {
        request_builder = request_builder.header("cookie", cookie_header);
      }
      if let Some(datadome_client_id) = auth.maybe_datadome_client_id.as_deref() {
        request_builder = request_builder.header("x-datadome-clientid", datadome_client_id);
      }
    }
    RequestCredential::Cookies { cookies, .. } => {
      request_builder = request_builder.header("cookie", cookies.as_header_value());
    }
  }

  match body {
    RequestBody::None => {}
    RequestBody::Json(body) => {
      let body_json = serde_json::to_string(body)
          .map_err(HiggsfieldClientError::RequestSerialization)?;
      debug!("Higgsfield request body: {}", body_json);
      request_builder = request_builder
          .header("content-type", "application/json")
          .body(body_json);
    }
    RequestBody::Form(body) => {
      request_builder = request_builder.form(body);
    }
  }

  let request = request_builder
      .build()
      .map_err(HiggsfieldClientError::WreqRequestBuild)?;

  info!("Higgsfield request: {} {} (browser: {})", method.as_str(), url, browser_profile.label());

  let response = client.execute(request)
      .await
      .map_err(HiggsfieldApiError::from_transport_error)?;

  let status = response.status();
  let protection_headers = ProtectionHeaders::from_headers(response.headers());

  let response_body = response.text()
      .await
      .map_err(HiggsfieldApiError::from_transport_error)?;

  info!("Higgsfield response: status={} ({} bytes)", status, response_body.len());
  debug!("Higgsfield response body: {}", response_body);

  classify_higgsfield_http_response(&HttpResponseSignals {
    status_code: status.as_u16(),
    body: &response_body,
    maybe_server_header: protection_headers.server.as_deref(),
    maybe_cf_ray: protection_headers.cf_ray.as_deref(),
    maybe_cf_mitigated: protection_headers.cf_mitigated.as_deref(),
    maybe_x_datadome: protection_headers.x_datadome.as_deref(),
    maybe_x_dd_b: protection_headers.x_dd_b.as_deref(),
    context: &url,
  })?;

  let parsed = serde_json::from_str::<Response>(&response_body)
      .map_err(|err| HiggsfieldApiError::deserialization(err, &response_body))?;

  Ok(parsed)
}

/// The response headers Cloudflare and DataDome use to announce themselves,
/// copied out before the body is consumed.
struct ProtectionHeaders {
  server: Option<String>,
  cf_ray: Option<String>,
  cf_mitigated: Option<String>,
  x_datadome: Option<String>,
  x_dd_b: Option<String>,
}

impl ProtectionHeaders {
  fn from_headers(headers: &HeaderMap) -> Self {
    let get = |name: &str| headers.get(name).and_then(|value| value.to_str().ok()).map(|s| s.to_string());
    Self {
      server: get("server"),
      cf_ray: get("cf-ray"),
      cf_mitigated: get("cf-mitigated"),
      x_datadome: get("x-datadome"),
      x_dd_b: get("x-dd-b"),
    }
  }
}
