use crate::client::browser_user_agents::FIREFOX_143_MAC_USER_AGENT;
use crate::datatypes::api::request_id::RequestId;
use crate::endpoint_bindings::generate_image_websocket::messages::websocket_client_message::{FastAspectRatio, QualityAspectRatio, WebsocketClientMessage};
use crate::endpoint_bindings::generate_image_websocket::messages::websocket_server_message::{ErrorMessage, ERR_CODE_RATE_LIMIT_EXCEEDED, WebsocketServerMessage};
use crate::error::grok_specific_api_error::GrokSpecificApiError;
use crate::error::grok_client_error::GrokClientError;
use crate::error::grok_error::GrokError;
use crate::error::grok_generic_api_error::GrokGenericApiError;
use crate::prompt_flags::PromptFlags;
use cloudflare_mitigation::headers::firefox_websocket_http_1_1_headers::get_firefox_websocket_http_1_1_headers;
use futures::TryStreamExt;
use log::warn;
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use wreq::header::{ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, COOKIE, ORIGIN, PRAGMA, SEC_WEBSOCKET_EXTENSIONS, USER_AGENT};
use wreq::ws::message::Message;
use wreq::ws::WebSocket;
use wreq::Client;
use wreq_util::Emulation;

const WEBSOCKET_URL: &str = "wss://grok.com/ws/imagine/listen";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Completed images expected per prompt when the caller doesn't say otherwise.
/// Mirrors the client message's `num_generations`.
pub const DEFAULT_IMAGE_COUNT: usize = 2;

/// How long each receive poll waits before looping (also the granularity at
/// which the shared socket lock is released for senders).
const RECEIVE_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Attempts (including the first) for a prompt send before giving up.
const SEND_ATTEMPTS: usize = 3;

/// A finished image produced by a prompt.
#[derive(Clone, Debug)]
pub struct CompletedImage {
  /// Identifies the prompt/task that produced the image. Sibling images from
  /// one prompt share it.
  pub request_id: RequestId,
  /// Where to download the image.
  pub url: String,
  /// The prompt the user typed.
  pub user_prompt: String,
  /// The model-enriched prompt.
  pub enriched_prompt: String,
}

/// One thing the imagine websocket told us while polling: a finished image,
/// or an error frame (e.g. out of quota) for a prompt.
#[derive(Clone, Debug)]
pub enum ImageWebsocketEvent {
  /// A finished (100%) image with a download URL.
  Completed(CompletedImage),
  /// Grok rejected or aborted a prompt. `request_id` is `None` when the frame
  /// didn't say which prompt it concerns.
  Failed {
    request_id: Option<RequestId>,
    /// User-facing message from the frame, if any.
    message: Option<String>,
    /// Grok's error code from the frame, if any.
    err_code: Option<String>,
    /// The raw frame, for logs.
    raw_frame: String,
  },
}

/// A live connection to Grok's "imagine" image websocket.
///
/// Cheap to clone and safe to share across tasks — the socket lives behind an
/// `Arc<Mutex<..>>`, so every method takes `&self`. Open one with [`connect`],
/// push prompts with [`send_fast_image_prompt`] / [`send_quality_image_prompt`]
/// (or the `_with_retry` variants, which reconnect on failure), read typed
/// frames with [`next_message`], or wait for finished images with
/// [`collect_images`].
///
/// [`connect`]: Self::connect
/// [`send_fast_image_prompt`]: Self::send_fast_image_prompt
/// [`send_quality_image_prompt`]: Self::send_quality_image_prompt
/// [`next_message`]: Self::next_message
/// [`collect_images`]: Self::collect_images
#[derive(Clone)]
pub struct GrokImageWebsocket {
  /// Kept so the socket can transparently reconnect after a dropped send.
  cookies: Arc<str>,
  socket: Arc<Mutex<WebSocket>>,
}

impl GrokImageWebsocket {
  /// Open a fresh imagine websocket authenticated with the given cookie header.
  pub async fn connect(cookies: &str) -> Result<Self, GrokError> {
    let socket = open_socket(cookies).await?;
    Ok(Self {
      cookies: Arc::from(cookies),
      socket: Arc::new(Mutex::new(socket)),
    })
  }

  /// Reconnect in place. All clones share the same handle, so they all see the
  /// new connection.
  pub async fn reconnect(&self) -> Result<(), GrokError> {
    let fresh = open_socket(&self.cookies).await?;
    *self.socket.lock().await = fresh;
    Ok(())
  }

  /// Send a **fast** ("speed") image prompt. Fast mode supports the
  /// [`FastAspectRatio`] set. `flags` appends any `--` long args (e.g.
  /// `--mode`) to the prompt; pass `&PromptFlags::default()` for none.
  ///
  /// Returns the prompt's request id, which every resulting frame carries.
  pub async fn send_fast_image_prompt(
    &self,
    prompt: &str,
    aspect_ratio: FastAspectRatio,
    flags: &PromptFlags,
  ) -> Result<RequestId, GrokError> {
    let message = WebsocketClientMessage::new_fast_image_prompt(prompt, aspect_ratio, flags);
    self.send_json(&message).await?;
    Ok(message.request_id())
  }

  /// [`send_fast_image_prompt`], reconnecting and retrying if the send fails
  /// (e.g. the socket dropped between prompts).
  ///
  /// [`send_fast_image_prompt`]: Self::send_fast_image_prompt
  pub async fn send_fast_image_prompt_with_retry(
    &self,
    prompt: &str,
    aspect_ratio: FastAspectRatio,
    flags: &PromptFlags,
  ) -> Result<RequestId, GrokError> {
    self.send_with_retry(WebsocketClientMessage::new_fast_image_prompt(prompt, aspect_ratio, flags)).await
  }

  /// Send a **quality** ("pro") image prompt — slower, higher quality. Quality
  /// mode supports the larger [`QualityAspectRatio`] set.
  pub async fn send_quality_image_prompt(
    &self,
    prompt: &str,
    aspect_ratio: QualityAspectRatio,
    flags: &PromptFlags,
  ) -> Result<RequestId, GrokError> {
    let message = WebsocketClientMessage::new_quality_image_prompt(prompt, aspect_ratio, flags);
    self.send_json(&message).await?;
    Ok(message.request_id())
  }

  /// [`send_quality_image_prompt`], reconnecting and retrying if the send
  /// fails.
  ///
  /// [`send_quality_image_prompt`]: Self::send_quality_image_prompt
  pub async fn send_quality_image_prompt_with_retry(
    &self,
    prompt: &str,
    aspect_ratio: QualityAspectRatio,
    flags: &PromptFlags,
  ) -> Result<RequestId, GrokError> {
    self.send_with_retry(WebsocketClientMessage::new_quality_image_prompt(prompt, aspect_ratio, flags)).await
  }

  /// Send a prepared message, reconnecting and retrying on failure.
  async fn send_with_retry(&self, message: WebsocketClientMessage) -> Result<RequestId, GrokError> {
    let mut last_error = None;

    for attempt in 1..=SEND_ATTEMPTS {
      match self.send_json(&message).await {
        Ok(()) => return Ok(message.request_id()),
        Err(err) => {
          warn!("Grok image prompt send failed (attempt {attempt}/{SEND_ATTEMPTS}): {err}");
          last_error = Some(err);
          if attempt < SEND_ATTEMPTS {
            self.reconnect().await?;
          }
        }
      }
    }

    Err(last_error.expect("loop runs at least once"))
  }

  /// Serialize and send any client message as a text frame.
  pub async fn send_json<T: Serialize>(&self, message: &T) -> Result<(), GrokError> {
    let json = serde_json::to_string(message)
        .map_err(GrokClientError::WebsocketRequestSerializationError)?;
    let mut socket = self.socket.lock().await;
    socket.send(Message::text(json))
        .await
        .map_err(GrokClientError::WebsocketSendError)?;
    Ok(())
  }

  /// Receive the next parsed server message, or `None` if `timeout` elapses,
  /// the stream closes, or a non-text frame arrives.
  pub async fn next_message(&self, timeout: Duration) -> Result<Option<WebsocketServerMessage>, GrokError> {
    match self.next_text(timeout).await? {
      Some(text) => Ok(Some(WebsocketServerMessage::from_json_str(&text)?)),
      None => Ok(None),
    }
  }

  /// Drain whatever the socket has to say for up to `budget`, returning the
  /// finished images and error frames seen (in order). Never blocks past the
  /// budget, so a polling thread can share one socket across many prompts and
  /// key each event on its request id. Progress / session / blob frames are
  /// skipped.
  ///
  /// Unlike [`collect_images`], an error frame is returned as an event rather
  /// than failing the call, since it concerns one prompt, not the socket.
  ///
  /// [`collect_images`]: Self::collect_images
  pub async fn poll_events(&self, budget: Duration) -> Result<Vec<ImageWebsocketEvent>, GrokError> {
    let deadline = Instant::now() + budget;
    let mut events = Vec::new();

    while Instant::now() < deadline {
      let remaining = deadline.saturating_duration_since(Instant::now()).min(RECEIVE_POLL_INTERVAL);
      let Some(text) = self.next_text(remaining).await? else {
        continue;
      };

      let Ok(message) = WebsocketServerMessage::from_json_str(&text) else {
        continue;
      };

      match &message {
        WebsocketServerMessage::Error(error) => events.push(ImageWebsocketEvent::Failed {
          request_id: error.request_id.clone().map(RequestId),
          message: error.err_msg.clone(),
          err_code: error.err_code.clone(),
          raw_frame: text,
        }),
        _ => {
          if let Some(image) = completed_image(&message) {
            events.push(ImageWebsocketEvent::Completed(image));
          }
        }
      }
    }

    Ok(events)
  }

  /// Wait for [`DEFAULT_IMAGE_COUNT`] completed images or until `timeout`.
  pub async fn collect_images(&self, timeout: Duration) -> Result<Vec<CompletedImage>, GrokError> {
    self.collect_n_images(DEFAULT_IMAGE_COUNT, timeout).await
  }

  /// Poll until `image_count` completed images arrive or `timeout` elapses.
  /// Non-image frames (progress updates, session notices, blobs) are skipped.
  pub async fn collect_n_images(
    &self,
    image_count: usize,
    timeout: Duration,
  ) -> Result<Vec<CompletedImage>, GrokError> {
    let deadline = Instant::now() + timeout;
    let mut images = Vec::new();

    while images.len() < image_count && Instant::now() < deadline {
      let text = match self.next_text(RECEIVE_POLL_INTERVAL).await? {
        Some(text) => text,
        None => continue,
      };

      // Parse leniently — the stream interleaves progress, session, and image
      // frames, and we only care about finished images.
      let Ok(message) = WebsocketServerMessage::from_json_str(&text) else {
        continue;
      };

      // An error frame (e.g. out-of-quota) means no images are coming; fail
      // fast with a typed error carrying the raw frame.
      if let WebsocketServerMessage::Error(error) = &message {
        return Err(error_frame_to_grok_error(error, &text));
      }

      if let Some(image) = completed_image(&message) {
        images.push(image);
      }
    }

    Ok(images)
  }

  /// Receive the next text frame, or `None` on timeout / close / non-text.
  async fn next_text(&self, timeout: Duration) -> Result<Option<String>, GrokError> {
    let mut socket = self.socket.lock().await;
    match tokio::time::timeout(timeout, socket.try_next()).await {
      Err(_elapsed) => Ok(None),
      Ok(result) => match result.map_err(GrokClientError::WebsocketReadError)? {
        Some(Message::Text(text)) => Ok(Some(text.to_string())),
        Some(_) => {
          warn!("Received non-text websocket message; ignoring.");
          Ok(None)
        }
        None => Ok(None),
      },
    }
  }
}

/// Map a server error frame (plus its raw body) to a typed error. Quota /
/// rate-limit exhaustion gets its own [`GrokSpecificApiError`] variant; any
/// other code is surfaced generically. Both carry the full raw frame.
fn error_frame_to_grok_error(error: &ErrorMessage, raw_frame: &str) -> GrokError {
  match error.err_code.as_deref() {
    Some(ERR_CODE_RATE_LIMIT_EXCEEDED) => {
      GrokSpecificApiError::ImageRateLimitExceeded { body: raw_frame.to_string() }.into()
    }
    _ => {
      GrokGenericApiError::UnexpectedWebsocketErrorFrame { body: raw_frame.to_string() }.into()
    }
  }
}

/// The completed-image view of a server message, if it is a finished image
/// (100%) that carries a URL.
fn completed_image(message: &WebsocketServerMessage) -> Option<CompletedImage> {
  let WebsocketServerMessage::Image(image) = message else {
    return None;
  };

  if image.percentage_complete? < 100.0 {
    return None;
  }

  Some(CompletedImage {
    request_id: RequestId(image.request_id.clone().unwrap_or_default()),
    url: image.url.clone()?,
    user_prompt: image.prompt.clone().unwrap_or_default(),
    enriched_prompt: image.full_prompt.clone().unwrap_or_default(),
  })
}

/// Open a raw imagine websocket. HTTP/1.1 websocket handshakes are sensitive to
/// header casing and order, and Cloudflare rejects lowercased names, so we send
/// an explicit, ordered header set (via `default_headers(false)`).
async fn open_socket(cookies: &str) -> Result<WebSocket, GrokError> {
  let client = Client::builder()
      .emulation(Emulation::Firefox143)
      .connection_verbose(true)
      .connect_timeout(CONNECT_TIMEOUT)
      .build()
      .map_err(GrokClientError::WreqClientError)?;

  let response = client.websocket(WEBSOCKET_URL)
      .default_headers(false)
      .orig_headers(get_firefox_websocket_http_1_1_headers())
      .header(COOKIE, cookies)
      .header(ORIGIN, "https://grok.com")
      .header(USER_AGENT, FIREFOX_143_MAC_USER_AGENT)
      .header(SEC_WEBSOCKET_EXTENSIONS, "permessage-deflate")
      .header(ACCEPT, "*/*")
      .header(ACCEPT_LANGUAGE, "en-US,en;q=0.5")
      .header("Sec-Fetch-Dest", "empty")
      .header("Sec-Fetch-Mode", "websocket")
      .header("Sec-Fetch-Site", "same-origin")
      .header(PRAGMA, "no-cache")
      .header(CACHE_CONTROL, "no-cache")
      .send()
      .await
      .map_err(GrokClientError::WreqClientError)?;

  let status = response.status();
  match status.as_u16() {
    101 => {}
    401 => return Err(GrokGenericApiError::LikelyWebsocketAuthentication401.into()),
    403 => return Err(GrokGenericApiError::LikelyWebsocketCloudflare403.into()),
    _ => return Err(GrokGenericApiError::UnexpectedWebsocketUpgradeStatusCode(status).into()),
  }

  response.into_websocket()
      .await
      .map_err(|err| GrokGenericApiError::WreqWebsocketUpgradeError(err).into())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::error::grok_generic_api_error::GrokGenericApiError;
  use crate::test_utils::grok_test_secrets::load_grok_test_secrets;
  use crate::test_utils::setup_test_logging::setup_test_logging;
  use errors::AnyhowResult;
  use log::{info, LevelFilter};

  // Cargo runs tests with the crate root as the working directory.
  fn ws_frame(file_name: &str) -> String {
    std::fs::read_to_string(format!("test_data/websocket_messages/{file_name}")).unwrap()
  }

  fn parse_frame(file_name: &str) -> WebsocketServerMessage {
    WebsocketServerMessage::from_json_str(&ws_frame(file_name)).unwrap()
  }

  // These exercise the handling that turns parsed frames into the `Ok`/`Err`
  // results `collect_images` yields, using real captured frames.

  #[test]
  fn completed_image_from_real_frame() {
    let image = completed_image(&parse_frame("real_image_complete.json"))
        .expect("a finished (100%) image with a url");

    assert_eq!(image.request_id.to_string(), "ab3fa8e9-92ed-4f1b-a350-7a897e264d54");
    assert!(image.url.starts_with("https://imagine-public.x.ai/"));
    assert_eq!(image.user_prompt, "A dead tree stump in the middle of a forest meadow");
    assert_eq!(image.enriched_prompt, "A dead tree stump in the middle of a forest meadow");
  }

  #[test]
  fn completed_image_ignores_progress_and_session_frames() {
    assert!(completed_image(&parse_frame("real_json_progress.json")).is_none());
    assert!(completed_image(&parse_frame("real_session_notice.json")).is_none());
  }

  #[test]
  fn rate_limit_error_frame_maps_to_quota_error() {
    let raw = ws_frame("real_rate_limit_error.json");
    let WebsocketServerMessage::Error(error) = parse_frame("real_rate_limit_error.json") else {
      panic!("expected an error frame");
    };

    match error_frame_to_grok_error(&error, &raw) {
      GrokError::ApiSpecific(GrokSpecificApiError::ImageRateLimitExceeded { body }) => {
        // The distinct error carries the full raw frame.
        assert!(body.contains("rate_limit_exceeded"));
        assert!(body.contains("Image rate limit exceeded"));
      }
      other => panic!("expected ImageRateLimitExceeded, got {:?}", other),
    }
  }

  #[test]
  fn unknown_error_code_maps_to_generic_error() {
    let raw = r#"{"type":"error","err_code":"some_new_code","err_msg":"nope"}"#;
    let WebsocketServerMessage::Error(error) = WebsocketServerMessage::from_json_str(raw).unwrap() else {
      panic!("expected an error frame");
    };

    match error_frame_to_grok_error(&error, raw) {
      GrokError::ApiGeneric(GrokGenericApiError::UnexpectedWebsocketErrorFrame { body }) => {
        assert_eq!(body, raw);
      }
      other => panic!("expected UnexpectedWebsocketErrorFrame, got {:?}", other),
    }
  }

  #[tokio::test]
  #[ignore] // Opens a real websocket and spends image-generation quota.
  async fn connect_prompt_and_collect() -> AnyhowResult<()> {
    setup_test_logging(LevelFilter::Info);
    let secrets = load_grok_test_secrets()?;

    let websocket = GrokImageWebsocket::connect(secrets.cookies.as_str()).await?;

    websocket.send_fast_image_prompt_with_retry(
      "A dinosaur on stilts walking on the beach",
      FastAspectRatio::WideThreeByTwo,
      &PromptFlags::default(),
    ).await?;

    match websocket.collect_images(Duration::from_secs(30)).await {
      Ok(images) => {
        info!("Collected {} image(s).", images.len());
        for image in &images {
          info!("Image: {} ({})", image.url, image.request_id.to_string());
        }
        assert!(!images.is_empty(), "expected at least one completed image");
      }
      // Out of quota is a valid "the plumbing works" outcome; surface it
      // clearly rather than failing.
      Err(GrokError::ApiSpecific(GrokSpecificApiError::ImageRateLimitExceeded { body })) => {
        info!("Image rate limit / quota exhausted (expected when out of quota): {body}");
      }
      Err(err) => return Err(err.into()),
    }

    Ok(())
  }
}
