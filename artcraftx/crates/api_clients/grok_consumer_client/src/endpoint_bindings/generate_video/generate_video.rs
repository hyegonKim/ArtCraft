//! `POST /rest/app-chat/conversations/new` — generate a video from an input
//! image (image-to-video). Captured 2026-08-23 in
//! `15_generate_video_from_generation.txt` (no prompt / "normal" mode) and
//! `16_generate_video_from_upload.txt` (with prompt / "custom" mode).
//!
//! The response is a stream of newline-delimited JSON: a conversation frame,
//! the echoed user message, then `streamingVideoGenerationResponse` frames
//! whose `progress` climbs 0 → 100. The final (100%) frame carries the
//! `videoUrl` / `assetId` / thumbnail. Reading the body waits for the whole
//! stream, i.e. until generation finishes.

use crate::client::grok_domain::GrokDomain;
use crate::credentials::grok_cookies::GrokCookies;
use crate::credentials::grok_request_headers::GrokRequestHeaders;
use crate::datatypes::api::file_id::FileId;
use crate::error::categorize_grok_http_error::categorize_grok_http_error;
use crate::error::grok_client_error::GrokClientError;
use crate::error::grok_error::GrokError;
use crate::error::grok_generic_api_error::GrokGenericApiError;
use crate::prompt_flags::{GenerationMode, PromptFlags};
use crate::utils::create_firefox_client::create_firefox_client;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use wreq::header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, COOKIE, ORIGIN, REFERER, USER_AGENT};

use crate::client::browser_user_agents::FIREFOX_143_MAC_USER_AGENT;

const NEW_CONVERSATION_PATH: &str = "/rest/app-chat/conversations/new";

/// The imagine video model.
const VIDEO_MODEL_NAME: &str = "imagine-video-gen";

const CONVERSATION_KIND_IMAGINE: &str = "CONVERSATION_KIND_IMAGINE";

/// Aspect ratios for generated video. Only `3:2` is confirmed from captures;
/// the others mirror the image aspect ratios and are included for parity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoAspectRatio {
  /// 1:1
  Square,
  /// 2:3
  TallTwoByThree,
  /// 3:2
  WideThreeByTwo,
  /// 16:9
  WideSixteenByNine,
  /// 9:16
  TallNineBySixteen,
}

impl VideoAspectRatio {
  pub fn as_grok_str(self) -> &'static str {
    match self {
      Self::Square => "1:1",
      Self::TallTwoByThree => "2:3",
      Self::WideThreeByTwo => "3:2",
      Self::WideSixteenByNine => "16:9",
      Self::TallNineBySixteen => "9:16",
    }
  }
}

/// Output resolution for generated video. `1080p` may be gated to certain
/// plans; the live `full_hd_no_audio_ten_seconds` test probes whether this
/// account can use it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoResolution {
  /// 480p
  Sd480p,
  /// 720p
  Hd720p,
  /// 1080p (may require a higher-tier plan)
  FullHd1080p,
}

impl VideoResolution {
  pub fn as_grok_str(self) -> &'static str {
    match self {
      Self::Sd480p => "480p",
      Self::Hd720p => "720p",
      Self::FullHd1080p => "1080p",
    }
  }
}

/// Length of the generated video. Grok offers a fixed set of durations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoDuration {
  /// 6 seconds
  SixSeconds,
  /// 10 seconds
  TenSeconds,
  /// 15 seconds
  FifteenSeconds,
}

impl VideoDuration {
  pub fn as_seconds(self) -> u32 {
    match self {
      Self::SixSeconds => 6,
      Self::TenSeconds => 10,
      Self::FifteenSeconds => 15,
    }
  }
}

/// What the video is generated from.
pub enum VideoSource<'a> {
  /// Text-to-video: generate from a prompt alone (no input image).
  Text { prompt: &'a str },
  /// Image-to-video: animate an input image asset (a Grok-generated image or
  /// an uploaded, untrusted one — the wire shape is identical). An optional
  /// prompt guides the motion.
  Image { input_asset_id: &'a str, prompt: Option<&'a str> },
}

pub struct GenerateVideoRequest<'a> {
  pub source: VideoSource<'a>,
  pub aspect_ratio: VideoAspectRatio,
  pub resolution: VideoResolution,
  pub duration: VideoDuration,

  /// Whether the video has generated audio. `false` sends `skipAudio: true`.
  pub enable_audio: bool,

  /// Optional `--` prompt flags (e.g. `--mode`). When `mode` is unset, the
  /// binding picks the source's default (`custom` for a prompt, `normal` for a
  /// bare image-to-video). See [`PromptFlags`].
  pub flags: PromptFlags,
}

pub struct GenerateVideoArgs<'a> {
  pub request: GenerateVideoRequest<'a>,
  pub credentials: &'a GrokCookies,
  /// Optional captured statsig/tracing headers (see [`GrokRequestHeaders`]).
  pub request_headers: Option<&'a GrokRequestHeaders>,
  pub domain_override: Option<&'a GrokDomain>,
  pub request_timeout: Option<Duration>,
}

/// The result of a completed generation stream.
#[derive(Clone, Debug, Default)]
pub struct GenerateVideoResponse {
  pub conversation_id: Option<String>,
  /// Available as soon as generation starts.
  pub video_id: Option<FileId>,
  /// Bucket path of the finished video (present once progress reaches 100).
  pub video_url: Option<String>,
  /// Bucket path of the preview image.
  pub thumbnail_url: Option<String>,
  pub asset_id: Option<String>,
  pub width: Option<u32>,
  pub height: Option<u32>,
  /// Highest progress seen in the stream (100 when finished).
  pub progress: Option<u32>,
}

/// Send the request and wait for the streamed generation to finish, returning
/// the completed [`GenerateVideoResponse`].
pub async fn generate_video(args: GenerateVideoArgs<'_>) -> Result<GenerateVideoResponse, GrokError> {
  let domain = args.domain_override.unwrap_or(&GrokDomain::DefaultDomain);
  let client = create_firefox_client()?;

  let mut request_builder = client.post(request_url(domain))
      .header(USER_AGENT, FIREFOX_143_MAC_USER_AGENT)
      .header(ACCEPT, "*/*")
      .header(ACCEPT_LANGUAGE, "en-US,en;q=0.5")
      .header(CONTENT_TYPE, "application/json")
      .header(ORIGIN, "https://grok.com")
      .header(REFERER, "https://grok.com/imagine")
      .header(COOKIE, args.credentials.to_string())
      .header("sec-fetch-dest", "empty")
      .header("sec-fetch-mode", "cors")
      .header("sec-fetch-site", "same-origin");

  if let Some(headers) = args.request_headers {
    request_builder = headers.apply(request_builder);
  }

  if let Some(timeout) = args.request_timeout {
    request_builder = request_builder.timeout(timeout);
  }

  let wire = GenerateVideoWireRequest::from_request(&args.request);

  let http_request = request_builder.json(&wire)
      .build()
      .map_err(|err| {
        error!("Error building generate_video request: {:?}", err);
        GrokClientError::WreqClientError(err)
      })?;

  let response = client.execute(http_request)
      .await
      .map_err(|err| {
        error!("Error sending generate_video request: {:?}", err);
        GrokGenericApiError::WreqError(err)
      })?;

  let status = response.status();
  info!("generate_video enqueue status: {}", status.as_u16());

  // Reading the body waits for the whole stream, i.e. until generation is done.
  let body = response.text()
      .await
      .map_err(|err| {
        error!("Error reading generate_video response body: {:?}", err);
        GrokGenericApiError::WreqError(err)
      })?;

  if !status.is_success() {
    error!("generate_video returned an error (code {}): {:?}", status.as_u16(), body);
    return Err(categorize_grok_http_error(status, Some(&body)));
  }

  Ok(parse_generate_video_stream(&body))
}

fn request_url(domain: &GrokDomain) -> String {
  format!("{}{}", domain.get_domain(), NEW_CONVERSATION_PATH)
}

/// Parse the newline-delimited JSON stream into a [`GenerateVideoResponse`].
/// Unparseable lines (e.g. a truncated final chunk) are skipped.
pub fn parse_generate_video_stream(body: &str) -> GenerateVideoResponse {
  let mut response = GenerateVideoResponse::default();

  for line in body.lines() {
    let line = line.trim();
    if line.is_empty() {
      continue;
    }

    let Ok(frame) = serde_json::from_str::<StreamFrame>(line) else {
      continue;
    };
    let Some(result) = frame.result else {
      continue;
    };

    if let Some(conversation) = result.conversation {
      if conversation.conversation_id.is_some() {
        response.conversation_id = conversation.conversation_id;
      }
    }

    if let Some(video) = result.response.and_then(|r| r.streaming_video) {
      if video.video_id.is_some() {
        response.video_id = video.video_id.map(FileId);
      }
      if video.progress.is_some() {
        response.progress = video.progress;
      }
      // The 100% frame carries the finished-video fields.
      if video.video_url.is_some() {
        response.video_url = video.video_url;
        response.asset_id = video.asset_id;
        response.thumbnail_url = video.thumbnail_url;
        response.width = video.width;
        response.height = video.height;
      }
    }
  }

  response
}

// ----- Wire request -----

/// Field order mirrors the web app's wire payload (observed 2026-08-23).
#[derive(Serialize)]
struct GenerateVideoWireRequest {
  #[serde(rename = "modelName")]
  model_name: &'static str,
  message: String,
  #[serde(rename = "enableImageStreaming")]
  enable_image_streaming: bool,
  #[serde(rename = "enableSideBySide")]
  enable_side_by_side: bool,
  #[serde(rename = "sendFinalMetadata")]
  send_final_metadata: bool,
  #[serde(rename = "responseMetadata")]
  response_metadata: ResponseMetadata,
  #[serde(rename = "mediaGenInput")]
  media_gen_input: MediaGenInput,
  kind: &'static str,
}

#[derive(Serialize)]
struct ResponseMetadata {
  experiments: Vec<serde_json::Value>,
  #[serde(rename = "modelConfigOverride")]
  model_config_override: ModelConfigOverride,
}

#[derive(Serialize)]
struct ModelConfigOverride {
  #[serde(rename = "modelMap")]
  model_map: ModelMap,
}

/// Serializes to an empty object `{}`.
#[derive(Serialize)]
struct ModelMap {}

#[derive(Serialize)]
struct MediaGenInput {
  #[serde(rename = "imageToVideo", skip_serializing_if = "Option::is_none")]
  image_to_video: Option<ImageToVideo>,
  #[serde(rename = "textToVideo", skip_serializing_if = "Option::is_none")]
  text_to_video: Option<TextToVideo>,
}

#[derive(Serialize)]
struct ImageToVideo {
  #[serde(rename = "prompt", skip_serializing_if = "Option::is_none")]
  prompt: Option<String>,
  #[serde(rename = "inputAssets")]
  input_assets: Vec<String>,
  #[serde(rename = "aspectRatio")]
  aspect_ratio: &'static str,
  duration: u32,
  #[serde(rename = "resolutionName")]
  resolution_name: &'static str,
  mode: &'static str,
  // Present only when audio is off; audio-on requests omit it (matching the
  // web app / captured requests).
  #[serde(rename = "skipAudio", skip_serializing_if = "is_false")]
  skip_audio: bool,
}

#[derive(Serialize)]
struct TextToVideo {
  prompt: String,
  #[serde(rename = "aspectRatio")]
  aspect_ratio: &'static str,
  duration: u32,
  #[serde(rename = "resolutionName")]
  resolution_name: &'static str,
  #[serde(rename = "skipAudio", skip_serializing_if = "is_false")]
  skip_audio: bool,
}

fn is_false(value: &bool) -> bool {
  !*value
}

impl GenerateVideoWireRequest {
  fn from_request(request: &GenerateVideoRequest) -> Self {
    let aspect_ratio = request.aspect_ratio.as_grok_str();
    let resolution_name = request.resolution.as_grok_str();
    let duration = request.duration.as_seconds();
    let skip_audio = !request.enable_audio;

    // The base prompt and default `--mode` come from the source (custom for a
    // prompt, normal for a bare image-to-video); a caller-supplied `flags.mode`
    // overrides it. The `message` is the prompt with all flags applied.
    let (base_prompt, default_mode): (&str, GenerationMode) = match &request.source {
      VideoSource::Text { prompt } => (prompt, GenerationMode::Custom),
      VideoSource::Image { prompt: Some(prompt), .. } => (prompt, GenerationMode::Custom),
      VideoSource::Image { prompt: None, .. } => ("", GenerationMode::Normal),
    };
    let mode = request.flags.mode.unwrap_or(default_mode);

    let mut flags = request.flags.clone();
    flags.mode = Some(mode);
    let message = flags.apply_to(base_prompt);

    let media_gen_input = match &request.source {
      VideoSource::Text { prompt } => MediaGenInput {
        image_to_video: None,
        text_to_video: Some(TextToVideo {
          prompt: prompt.to_string(),
          aspect_ratio,
          duration,
          resolution_name,
          skip_audio,
        }),
      },
      VideoSource::Image { input_asset_id, prompt } => MediaGenInput {
        image_to_video: Some(ImageToVideo {
          prompt: prompt.map(str::to_string),
          input_assets: vec![input_asset_id.to_string()],
          aspect_ratio,
          duration,
          resolution_name,
          mode: mode.as_flag_value(),
          skip_audio,
        }),
        text_to_video: None,
      },
    };

    Self {
      model_name: VIDEO_MODEL_NAME,
      message,
      enable_image_streaming: true,
      enable_side_by_side: true,
      send_final_metadata: true,
      response_metadata: ResponseMetadata {
        experiments: Vec::new(),
        model_config_override: ModelConfigOverride { model_map: ModelMap {} },
      },
      media_gen_input,
      kind: CONVERSATION_KIND_IMAGINE,
    }
  }
}

// ----- Wire response (streamed frames) -----

#[derive(Deserialize)]
struct StreamFrame {
  result: Option<StreamResult>,
}

#[derive(Deserialize)]
struct StreamResult {
  conversation: Option<StreamConversation>,
  response: Option<StreamResponse>,
}

#[derive(Deserialize)]
struct StreamConversation {
  #[serde(rename = "conversationId")]
  conversation_id: Option<String>,
}

#[derive(Deserialize)]
struct StreamResponse {
  #[serde(rename = "streamingVideoGenerationResponse")]
  streaming_video: Option<StreamingVideo>,
}

#[derive(Deserialize)]
struct StreamingVideo {
  #[serde(rename = "videoId")]
  video_id: Option<String>,
  progress: Option<u32>,
  #[serde(rename = "assetId")]
  asset_id: Option<String>,
  #[serde(rename = "videoUrl")]
  video_url: Option<String>,
  #[serde(rename = "thumbnailImageUrl")]
  thumbnail_url: Option<String>,
  width: Option<u32>,
  height: Option<u32>,
}

#[cfg(test)]
mod tests {
  use super::*;

  // Cargo runs tests with the crate root as the working directory.
  fn load(file_name: &str) -> String {
    std::fs::read_to_string(format!("test_data/endpoint_responses/{file_name}")).unwrap()
  }

  mod wire_format_tests {
    use super::*;

    #[test]
    fn url_uses_domain() {
      assert_eq!(
        request_url(&GrokDomain::DefaultDomain),
        "https://grok.com/rest/app-chat/conversations/new",
      );
      let custom = GrokDomain::Custom("http://localhost:8080".to_string());
      assert_eq!(request_url(&custom), "http://localhost:8080/rest/app-chat/conversations/new");
    }

    #[test]
    fn aspect_ratio_and_resolution_strings() {
      assert_eq!(VideoAspectRatio::Square.as_grok_str(), "1:1");
      assert_eq!(VideoAspectRatio::TallTwoByThree.as_grok_str(), "2:3");
      assert_eq!(VideoAspectRatio::WideThreeByTwo.as_grok_str(), "3:2");
      assert_eq!(VideoAspectRatio::WideSixteenByNine.as_grok_str(), "16:9");
      assert_eq!(VideoAspectRatio::TallNineBySixteen.as_grok_str(), "9:16");
      assert_eq!(VideoResolution::Sd480p.as_grok_str(), "480p");
      assert_eq!(VideoResolution::Hd720p.as_grok_str(), "720p");
      assert_eq!(VideoResolution::FullHd1080p.as_grok_str(), "1080p");
      assert_eq!(VideoDuration::SixSeconds.as_seconds(), 6);
      assert_eq!(VideoDuration::TenSeconds.as_seconds(), 10);
      assert_eq!(VideoDuration::FifteenSeconds.as_seconds(), 15);
    }

    // Audio off adds `skipAudio: true`; audio on omits the field entirely.
    #[test]
    fn skip_audio_only_serialized_when_audio_off() {
      let no_audio = GenerateVideoRequest {
        source: VideoSource::Text { prompt: "x" },
        aspect_ratio: VideoAspectRatio::WideThreeByTwo,
        resolution: VideoResolution::FullHd1080p,
        duration: VideoDuration::TenSeconds,
        enable_audio: false,
        flags: PromptFlags::default(),
      };
      let value = serde_json::to_value(GenerateVideoWireRequest::from_request(&no_audio)).unwrap();
      let text_to_video = &value["mediaGenInput"]["textToVideo"];
      assert_eq!(text_to_video["skipAudio"], serde_json::json!(true));
      assert_eq!(text_to_video["duration"], serde_json::json!(10));
      assert_eq!(text_to_video["resolutionName"], serde_json::json!("1080p"));

      let with_audio = GenerateVideoRequest { enable_audio: true, ..no_audio };
      let value = serde_json::to_value(GenerateVideoWireRequest::from_request(&with_audio)).unwrap();
      assert!(value["mediaGenInput"]["textToVideo"].get("skipAudio").is_none());
    }

    // A caller-supplied `--mode` flows into the text-to-video message.
    #[test]
    fn caller_mode_flag_flows_into_message() {
      let request = GenerateVideoRequest {
        source: VideoSource::Text { prompt: "woman on beach" },
        aspect_ratio: VideoAspectRatio::WideThreeByTwo,
        resolution: VideoResolution::Sd480p,
        duration: VideoDuration::SixSeconds,
        enable_audio: true,
        flags: PromptFlags::with_mode(GenerationMode::Spicy),
      };
      let value = serde_json::to_value(GenerateVideoWireRequest::from_request(&request)).unwrap();
      assert_eq!(value["message"], "woman on beach --mode=extremely-spicy-or-crazy");
    }

    // For image-to-video, a caller mode overrides the default in BOTH the
    // message text and the structured `mode` field.
    #[test]
    fn caller_mode_overrides_default_for_image_to_video() {
      let request = GenerateVideoRequest {
        source: VideoSource::Image { input_asset_id: "abc", prompt: None },
        aspect_ratio: VideoAspectRatio::WideThreeByTwo,
        resolution: VideoResolution::Sd480p,
        duration: VideoDuration::SixSeconds,
        enable_audio: true,
        flags: PromptFlags::with_mode(GenerationMode::Spicy),
      };
      let value = serde_json::to_value(GenerateVideoWireRequest::from_request(&request)).unwrap();
      assert_eq!(value["message"], "--mode=extremely-spicy-or-crazy");
      assert_eq!(value["mediaGenInput"]["imageToVideo"]["mode"], "extremely-spicy-or-crazy");
    }

    // Real image-to-video "from generation" send frame (no prompt, normal, 480p).
    #[test]
    fn image_to_video_normal_mode_matches_capture() {
      let request = GenerateVideoRequest {
        source: VideoSource::Image {
          input_asset_id: "11111111-1111-4111-8111-111111111111",
          prompt: None,
        },
        aspect_ratio: VideoAspectRatio::WideThreeByTwo,
        resolution: VideoResolution::Sd480p,
        duration: VideoDuration::SixSeconds,
        enable_audio: true,
        flags: PromptFlags::default(),
      };
      let ours = serde_json::to_value(GenerateVideoWireRequest::from_request(&request)).unwrap();
      let captured: serde_json::Value =
          serde_json::from_str(&load("generate_video_from_generation_request.json")).unwrap();
      assert_eq!(ours, captured);
    }

    // Real image-to-video "from upload" send frame (with prompt, custom, 720p).
    #[test]
    fn image_to_video_custom_mode_matches_capture() {
      let request = GenerateVideoRequest {
        source: VideoSource::Image {
          input_asset_id: "22222222-2222-4222-8222-222222222222",
          prompt: Some("Windmill spins as the camera orbits the night scene. Stars above."),
        },
        aspect_ratio: VideoAspectRatio::WideThreeByTwo,
        resolution: VideoResolution::Hd720p,
        duration: VideoDuration::SixSeconds,
        enable_audio: true,
        flags: PromptFlags::default(),
      };
      let ours = serde_json::to_value(GenerateVideoWireRequest::from_request(&request)).unwrap();
      let captured: serde_json::Value =
          serde_json::from_str(&load("generate_video_from_upload_request.json")).unwrap();
      assert_eq!(ours, captured);
    }

    // Real text-to-video send frame (prompt only, 2:3, 480p) — capture 18.
    #[test]
    fn text_to_video_matches_capture() {
      let request = GenerateVideoRequest {
        source: VideoSource::Text { prompt: "An asteroid hits the city." },
        aspect_ratio: VideoAspectRatio::TallTwoByThree,
        resolution: VideoResolution::Sd480p,
        duration: VideoDuration::SixSeconds,
        enable_audio: true,
        flags: PromptFlags::default(),
      };
      let ours = serde_json::to_value(GenerateVideoWireRequest::from_request(&request)).unwrap();
      let captured: serde_json::Value =
          serde_json::from_str(&load("generate_video_from_text_request.json")).unwrap();
      assert_eq!(ours, captured);
    }
  }

  mod response_parsing_tests {
    use super::*;

    #[test]
    fn parses_real_streamed_response() {
      let response = parse_generate_video_stream(&load("generate_video_response.txt"));

      assert_eq!(
        response.conversation_id.as_deref(),
        Some("8b592ec5-b3ec-4cb4-b1f3-ddfd87b20625"),
      );
      assert_eq!(
        response.video_id.as_ref().map(|id| id.0.as_str()),
        Some("1622a562-971d-479b-b4be-44c96f3cd813"),
      );
      assert_eq!(response.progress, Some(100));
      assert!(response.video_url.as_deref().unwrap().ends_with("generated_video.mp4"));
      assert_eq!(
        response.asset_id.as_deref(),
        Some("1622a562-971d-479b-b4be-44c96f3cd813"),
      );
      assert!(response.thumbnail_url.as_deref().unwrap().ends_with("preview_image.jpg"));
      assert_eq!(response.width, Some(832));
      assert_eq!(response.height, Some(352));
    }

    #[test]
    fn truncated_lines_are_skipped() {
      // A conversation frame plus a garbage/truncated line.
      let body = "{\"result\":{\"conversation\":{\"conversationId\":\"abc\"}}}\n{\"result\":{\"resp";
      let response = parse_generate_video_stream(body);
      assert_eq!(response.conversation_id.as_deref(), Some("abc"));
      assert!(response.video_id.is_none());
    }
  }

  mod real_wire_tests {
    use super::*;
    use crate::endpoint_bindings::list_assets::list_assets::{ListAssetsArgs, ListAssetsRequest};
    use crate::endpoint_bindings::upload_file::grok_upload_file::{grok_upload_file, GrokUploadFileArgs, GrokUploadFileRequest, PathOrFile};
    use crate::error::grok_specific_api_error::GrokSpecificApiError;
    use crate::test_utils::grok_test_secrets::load_grok_test_secrets;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    use errors::AnyhowResult;
    use log::{info, LevelFilter};
    use std::path::Path;

    const TEST_IMAGE_PATH: &str = "test_data/images/test_upload.png";
    const VIDEO_TIMEOUT: Duration = Duration::from_secs(180);

    /// Headers carrying a real, browser-minted statsig for this endpoint.
    ///
    /// Grok rejects headlessly-computed signatures, so these tests need one
    /// harvested from a real browser (see the `harvest_statsig` binary /
    /// `grok_consumer_statsig`). Pass it via `GROK_STATSIG=<x-statsig-id>`; it is
    /// time-bound, so harvest it right before running. The fallback is a stale
    /// placeholder that will 403 with a clear message.
    fn captured_statsig_headers() -> GrokRequestHeaders {
      let statsig = std::env::var("GROK_STATSIG").unwrap_or_else(|_|
        "PASTE_A_FRESH_x-statsig-id_OR_SET_GROK_STATSIG".to_string());
      GrokRequestHeaders {
        statsig_id: Some(statsig),
        ..Default::default()
      }
    }

    /// Generate a video from `source` at the given resolution/duration/audio and
    /// assert a finished video URL comes back. A statsig rejection surfaces as
    /// [`GrokSpecificApiError::StatsigSignatureRejected`].
    async fn generate_and_assert(
      source: VideoSource<'_>,
      resolution: VideoResolution,
      duration: VideoDuration,
      enable_audio: bool,
      headers: &GrokRequestHeaders,
    ) -> AnyhowResult<()> {
      let secrets = load_grok_test_secrets()?;

      let result = generate_video(GenerateVideoArgs {
        request: GenerateVideoRequest {
          source,
          aspect_ratio: VideoAspectRatio::TallTwoByThree,
          resolution,
          duration,
          enable_audio,
          flags: PromptFlags::default(),
        },
        credentials: &secrets.cookies,
        request_headers: Some(headers),
        domain_override: None,
        request_timeout: Some(VIDEO_TIMEOUT),
      }).await;

      match result {
        Ok(response) => {
          info!("generate_video response: {:?}", response);
          assert_eq!(response.progress, Some(100), "video did not finish");
          assert!(response.video_url.is_some(), "expected a finished video url");
          Ok(())
        }
        Err(GrokError::ApiSpecific(GrokSpecificApiError::StatsigSignatureRejected { status_code, body })) => {
          panic!("x-statsig-id rejected (HTTP {status_code}). Body: {body}");
        }
        Err(err) => Err(err.into()),
      }
    }

    // Way 1: text-to-video (from a text prompt alone). Uses a browser-harvested
    // statsig (set `GROK_STATSIG`); this is the isolation test that a fresh
    // signature drives the whole video request end-to-end.
    #[tokio::test]
    #[ignore] // Hits the real website; spends video quota; needs a fresh GROK_STATSIG.
    async fn generate_video_from_text_prompt() -> AnyhowResult<()> {
      setup_test_logging(LevelFilter::Info);
      generate_and_assert(
        VideoSource::Text { prompt: "a lone lighthouse on a cliff at dusk, the light beam sweeps across the water" },
        VideoResolution::Sd480p,
        VideoDuration::SixSeconds,
        true,
        &captured_statsig_headers(),
      ).await
    }

    // 1080p, audio off, 10 seconds — exercises the new resolution/duration/audio
    // controls end-to-end. 1080p is plan-gated; on a lower tier the server
    // returns a concrete `VideoResolutionNotAllowed` (verified here) instead of
    // a silent failure. A finished video is also accepted (higher-tier plans).
    #[tokio::test]
    #[ignore] // Hits the real website; spends video quota; needs a fresh GROK_STATSIG.
    async fn full_hd_no_audio_ten_seconds() -> AnyhowResult<()> {
      setup_test_logging(LevelFilter::Info);
      let secrets = load_grok_test_secrets()?;
      let headers = captured_statsig_headers();

      let result = generate_video(GenerateVideoArgs {
        request: GenerateVideoRequest {
          source: VideoSource::Text { prompt: "a red kite flying over a green field" },
          aspect_ratio: VideoAspectRatio::WideThreeByTwo,
          resolution: VideoResolution::FullHd1080p,
          duration: VideoDuration::TenSeconds,
          enable_audio: false,
          flags: PromptFlags::default(),
        },
        credentials: &secrets.cookies,
        request_headers: Some(&headers),
        domain_override: None,
        request_timeout: Some(VIDEO_TIMEOUT),
      }).await;

      match result {
        Ok(response) => {
          info!("1080p video finished (plan allows it): {:?}", response);
          assert_eq!(response.progress, Some(100), "video did not finish");
          assert!(response.video_url.is_some(), "expected a finished video url");
          Ok(())
        }
        Err(GrokError::ApiSpecific(GrokSpecificApiError::VideoResolutionNotAllowed { status_code, body })) => {
          info!("1080p correctly reported as plan-gated (HTTP {status_code}): {body}");
          Ok(())
        }
        Err(GrokError::ApiSpecific(GrokSpecificApiError::StatsigSignatureRejected { status_code, body })) => {
          panic!("x-statsig-id rejected (HTTP {status_code}) — harvest a fresh GROK_STATSIG. Body: {body}");
        }
        Err(err) => Err(err.into()),
      }
    }

    // Way 2: from an existing generated image (fetched from the asset list).
    #[tokio::test]
    #[ignore] // Hits the real website; spends video quota.
    async fn generate_video_from_generated_image() -> AnyhowResult<()> {
      setup_test_logging(LevelFilter::Info);
      let secrets = load_grok_test_secrets()?;

      let assets = ListAssetsArgs {
        request: ListAssetsRequest { page_size: Some(20) },
        credentials: &secrets.cookies,
        domain_override: None,
        request_headers: Some(&secrets.headers),
        request_timeout: None,
      }.send().await?;

      let image = assets.assets.iter()
          .find(|a| a.mime_type.as_deref() == Some("image/jpeg"))
          .expect("expected at least one generated image asset");

      generate_and_assert(
        VideoSource::Image { input_asset_id: &image.asset_id, prompt: None },
        VideoResolution::Sd480p,
        VideoDuration::SixSeconds,
        true,
        &captured_statsig_headers(),
      ).await
    }

    // Way 3: from an uploaded (untrusted) image.
    #[tokio::test]
    #[ignore] // Hits the real website; spends video quota.
    async fn generate_video_from_uploaded_image() -> AnyhowResult<()> {
      setup_test_logging(LevelFilter::Info);
      let secrets = load_grok_test_secrets()?;

      let upload = grok_upload_file(GrokUploadFileArgs {
        request: GrokUploadFileRequest { file: PathOrFile::Path(Path::new(TEST_IMAGE_PATH)) },
        cookie: secrets.cookies.as_str(),
        domain_override: None,
        request_timeout: Some(Duration::from_secs(30)),
      }).await?;
      let asset_id = upload.file_id.expect("upload should yield a file id").0;

      generate_and_assert(
        VideoSource::Image { input_asset_id: &asset_id, prompt: Some("slow cinematic zoom") },
        VideoResolution::Sd480p,
        VideoDuration::SixSeconds,
        true,
        &captured_statsig_headers(),
      ).await
    }
  }
}
