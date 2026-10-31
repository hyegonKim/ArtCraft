//! POST `/fnf/jobs/v2/gpt_image_2` — enqueue a GPT Image 2 job (the web
//! app's "GPT Image 2"; job set type `gpt_image_2`).
//!
//! Option sets below were read off the web app's image generator on
//! 2026-08-31: 9 aspect ratios (incl. Auto), Low/Medium/High quality,
//! 1K (1024px) / 2K (2048px) / 4K (4096px), 1–4 images.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::enqueue_jobs_response::EnqueueJobsResponse;
use crate::types::image_quality::ImageQuality;
use crate::types::image_aspect_ratio::ImageAspectRatio;
use crate::types::image_batch_size::ImageBatchSize;
use crate::types::image_dimensions::ImageDimensions;
use crate::types::image_resolution::ImageResolution;
use crate::types::media_input::MediaInput;
use crate::types::media_reference::MediaReference;
use crate::types::string_enum::string_enum;
use serde::Serialize;

const PATH: &str = "/fnf/jobs/v2/gpt_image_2";

/// The `model` field the web app sends on this endpoint.
const MODEL: &str = "gpt_image_2";

string_enum! {
  /// The backend variant behind GPT Image 2. The web app currently always
  /// sends `videotape-alpha`; it's echoed back as `params.model`.
  GptImage2SubModel {
    VideotapeAlpha => "videotape-alpha",
  }
}

impl Default for GptImage2SubModel {
  fn default() -> Self {
    Self::VideotapeAlpha
  }
}

/// The aspect ratios the web app offers for GPT Image 2, in its menu order.
/// (No 4:5 / 5:4, unlike Nano Banana Pro.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GptImage2AspectRatio {
  /// Let the model pick (meant for reference-image workflows).
  Auto,
  Square1x1,
  Landscape3x2,
  Portrait2x3,
  Landscape16x9,
  Portrait9x16,
  Landscape4x3,
  Portrait3x4,
  Landscape21x9,
}

impl GptImage2AspectRatio {
  pub fn all() -> [Self; 9] {
    [
      Self::Auto, Self::Square1x1, Self::Landscape3x2, Self::Portrait2x3, Self::Landscape16x9,
      Self::Portrait9x16, Self::Landscape4x3, Self::Portrait3x4, Self::Landscape21x9,
    ]
  }

  /// The wire vocabulary value.
  pub fn to_image_aspect_ratio(self) -> ImageAspectRatio {
    match self {
      Self::Auto => ImageAspectRatio::Auto,
      Self::Square1x1 => ImageAspectRatio::Square1x1,
      Self::Landscape3x2 => ImageAspectRatio::Landscape3x2,
      Self::Portrait2x3 => ImageAspectRatio::Portrait2x3,
      Self::Landscape16x9 => ImageAspectRatio::Landscape16x9,
      Self::Portrait9x16 => ImageAspectRatio::Portrait9x16,
      Self::Landscape4x3 => ImageAspectRatio::Landscape4x3,
      Self::Portrait3x4 => ImageAspectRatio::Portrait3x4,
      Self::Landscape21x9 => ImageAspectRatio::Landscape21x9,
    }
  }

  pub fn as_str(self) -> &'static str {
    match self {
      Self::Auto => "auto",
      Self::Square1x1 => "1:1",
      Self::Landscape3x2 => "3:2",
      Self::Portrait2x3 => "2:3",
      Self::Landscape16x9 => "16:9",
      Self::Portrait9x16 => "9:16",
      Self::Landscape4x3 => "4:3",
      Self::Portrait3x4 => "3:4",
      Self::Landscape21x9 => "21:9",
    }
  }
}

impl Serialize for GptImage2AspectRatio {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.as_str())
  }
}

/// Quality tiers the web app offers for GPT Image 2 ("Fastest and
/// cheapest" / "Balanced visuals" / "Best visual fidelity").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum GptImage2Quality {
  Low,
  #[default]
  Medium,
  High,
}

impl GptImage2Quality {
  pub fn all() -> [Self; 3] {
    [Self::Low, Self::Medium, Self::High]
  }

  pub fn to_image_quality(self) -> ImageQuality {
    match self {
      Self::Low => ImageQuality::Low,
      Self::Medium => ImageQuality::Medium,
      Self::High => ImageQuality::High,
    }
  }

  pub fn as_str(self) -> &'static str {
    match self {
      Self::Low => "low",
      Self::Medium => "medium",
      Self::High => "high",
    }
  }
}

impl Serialize for GptImage2Quality {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.as_str())
  }
}

/// Resolution tiers the web app offers for GPT Image 2, with the pixel
/// size it labels each with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum GptImage2Resolution {
  /// 1024px
  #[default]
  OneK,
  /// 2048px
  TwoK,
  /// 4096px
  FourK,
}

impl GptImage2Resolution {
  pub fn all() -> [Self; 3] {
    [Self::OneK, Self::TwoK, Self::FourK]
  }

  /// The pixel size the web app's menu shows for the tier.
  pub fn labeled_pixels(self) -> u32 {
    match self {
      Self::OneK => 1024,
      Self::TwoK => 2048,
      Self::FourK => 4096,
    }
  }

  pub fn to_image_resolution(self) -> ImageResolution {
    match self {
      Self::OneK => ImageResolution::OneK,
      Self::TwoK => ImageResolution::TwoK,
      Self::FourK => ImageResolution::FourK,
    }
  }

  pub fn as_str(self) -> &'static str {
    match self {
      Self::OneK => "1k",
      Self::TwoK => "2k",
      Self::FourK => "4k",
    }
  }
}

impl Serialize for GptImage2Resolution {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.as_str())
  }
}

pub struct GptImage2Args<'a> {
  pub request: GptImage2Request,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// The material part of a GPT Image 2 request. Serializable so it can be
/// logged or persisted separately from the session.
#[derive(Clone, Debug, Serialize)]
pub struct GptImage2Request {
  pub prompt: String,

  pub aspect_ratio: GptImage2AspectRatio,

  pub quality: GptImage2Quality,

  pub resolution: GptImage2Resolution,

  /// How many images to generate (1–4). Each costs credits.
  pub batch_size: ImageBatchSize,

  /// Reference images (image-to-image), uploaded first via
  /// `endpoints::media` / `HiggsfieldSession::upload_reference_media`.
  /// Empty for text-to-image. Sent as `medias` with role `image`.
  ///
  /// The web app refuses references under 300px on a side for this model
  /// ("Image is too small"). With a reference and `Auto` aspect it sends
  /// the reference's pixel size as `width`/`height`; this client keeps its
  /// derived size (or `maybe_dimensions`).
  pub reference_images: Vec<MediaInput>,

  pub sub_model: GptImage2SubModel,

  /// Spend from the plan's "unlimited" pool instead of credits, if the plan
  /// has one.
  pub use_unlim: bool,

  /// Override the pixel size sent with the request. When `None`, the size
  /// the web app would send for `aspect_ratio` + `resolution` is used.
  pub maybe_dimensions: Option<ImageDimensions>,
}

impl GptImage2Request {
  /// A text-to-image request with the web app's defaults (1 image, credits).
  pub fn text_to_image(
    prompt: impl Into<String>,
    aspect_ratio: GptImage2AspectRatio,
    quality: GptImage2Quality,
    resolution: GptImage2Resolution,
  ) -> Self {
    Self {
      prompt: prompt.into(),
      aspect_ratio,
      quality,
      resolution,
      batch_size: ImageBatchSize::One,
      reference_images: Vec::new(),
      sub_model: GptImage2SubModel::default(),
      use_unlim: false,
      maybe_dimensions: None,
    }
  }

  /// Add reference images (image-to-image).
  pub fn with_reference_images(mut self, reference_images: Vec<MediaInput>) -> Self {
    self.reference_images = reference_images;
    self
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.prompt.trim().is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("prompt is empty".to_string()));
    }
    Ok(())
  }

  fn dimensions(&self) -> Result<ImageDimensions, HiggsfieldClientError> {
    if let Some(dimensions) = self.maybe_dimensions {
      return Ok(dimensions);
    }
    ImageDimensions::for_aspect_ratio(&self.aspect_ratio.to_image_aspect_ratio(), &self.resolution.to_image_resolution())
        .ok_or_else(|| HiggsfieldClientError::InvalidRequest(format!(
          "can't derive dimensions for aspect ratio {} at {}; pass maybe_dimensions",
          self.aspect_ratio.as_str(), self.resolution.as_str(),
        )))
  }

  fn to_body(&self) -> Result<GptImage2RequestBody, HiggsfieldClientError> {
    let dimensions = self.dimensions()?;
    Ok(GptImage2RequestBody {
      params: GptImage2Params {
        prompt: self.prompt.clone(),
        aspect_ratio: self.aspect_ratio,
        quality: self.quality,
        resolution: self.resolution,
        sub_model: self.sub_model.clone(),
        batch_size: self.batch_size,
        model: MODEL,
        width: dimensions.width,
        height: dimensions.height,
        medias: self.reference_images.iter().cloned().map(MediaReference::image).collect(),
      },
      use_unlim: self.use_unlim,
    })
  }
}

/// Enqueue the job. The response's job ids are what to poll (see
/// `endpoints::jobs`).
pub async fn gpt_image_2(args: GptImage2Args<'_>) -> Result<EnqueueJobsResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body()?;
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct GptImage2RequestBody {
  params: GptImage2Params,
  use_unlim: bool,
}

#[derive(Serialize)]
struct GptImage2Params {
  prompt: String,
  aspect_ratio: GptImage2AspectRatio,
  quality: GptImage2Quality,
  resolution: GptImage2Resolution,
  sub_model: GptImage2SubModel,
  batch_size: ImageBatchSize,
  model: &'static str,
  width: u32,
  height: u32,
  medias: Vec<MediaReference>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::{json, Value};

  // ── Option sets ──

  #[test]
  fn aspect_ratios_match_the_web_app_menu() {
    let wire: Vec<&str> = GptImage2AspectRatio::all().iter().map(|a| a.as_str()).collect();
    assert_eq!(wire, ["auto", "1:1", "3:2", "2:3", "16:9", "9:16", "4:3", "3:4", "21:9"]);
    for ratio in GptImage2AspectRatio::all() {
      assert_eq!(ratio.to_image_aspect_ratio().as_str(), ratio.as_str());
    }
  }

  #[test]
  fn quality_and_resolution_match_the_web_app_menus() {
    let qualities: Vec<&str> = GptImage2Quality::all().iter().map(|q| q.as_str()).collect();
    assert_eq!(qualities, ["low", "medium", "high"]);
    for quality in GptImage2Quality::all() {
      assert_eq!(quality.to_image_quality().as_str(), quality.as_str());
    }

    let resolutions: Vec<(&str, u32)> = GptImage2Resolution::all().iter().map(|r| (r.as_str(), r.labeled_pixels())).collect();
    assert_eq!(resolutions, [("1k", 1024), ("2k", 2048), ("4k", 4096)]);
  }

  // ── Outbound shape ──

  #[test]
  fn wire_body_matches_captured_request() {
    // Captured from the web app: 9:16 at 2k, high quality, one image.
    let request = GptImage2Request::text_to_image("a corgi on a bike", GptImage2AspectRatio::Portrait9x16, GptImage2Quality::High, GptImage2Resolution::TwoK);
    let actual: Value = serde_json::to_value(request.to_body().unwrap()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"9:16","quality":"high","resolution":"2k","sub_model":"videotape-alpha","batch_size":1,"model":"gpt_image_2","width":1152,"height":2048,"medias":[]},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn wire_body_with_reference_image_matches_captured_request() {
    // Captured from the web app 2026-08-31 (ids scrubbed): Auto aspect, low
    // quality, 4k, one reference as `medias[].role = "image"`. The web app
    // sent the reference's own 640x640 as width/height; this client sends
    // its derived Auto size (or `maybe_dimensions`).
    let request = GptImage2Request::text_to_image("a corgi on a bike", GptImage2AspectRatio::Auto, GptImage2Quality::Low, GptImage2Resolution::FourK)
        .with_reference_images(vec![MediaInput::uploaded("00000000-0000-4000-8000-0000000000aa", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png")]);
    let actual: Value = serde_json::to_value(request.to_body().unwrap()).unwrap();
    let dimensions = request.dimensions().unwrap();
    let expected_json = r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"auto","quality":"low","resolution":"4k","sub_model":"videotape-alpha","batch_size":1,"model":"gpt_image_2","width":W,"height":H,"medias":[{"role":"image","data":{"id":"00000000-0000-4000-8000-0000000000aa","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png"}}]},"use_unlim":false}"#
        .replace("\"width\":W", &format!("\"width\":{}", dimensions.width)).replace("\"height\":H", &format!("\"height\":{}", dimensions.height));
    let expected: Value = serde_json::from_str(&expected_json).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn every_option_combination_derives_dimensions() {
    for ratio in GptImage2AspectRatio::all() {
      for resolution in GptImage2Resolution::all() {
        let request = GptImage2Request::text_to_image("p", ratio, GptImage2Quality::Low, resolution);
        request.dimensions().unwrap_or_else(|err| panic!("{} @ {}: {err}", ratio.as_str(), resolution.as_str()));
      }
    }
  }

  #[test]
  fn public_request_serializes_with_typed_enums() {
    let mut request = GptImage2Request::text_to_image("a cat", GptImage2AspectRatio::Square1x1, GptImage2Quality::Medium, GptImage2Resolution::OneK);
    request.batch_size = ImageBatchSize::Two;
    let value = serde_json::to_value(&request).unwrap();
    assert_eq!(value["quality"], json!("medium"));
    assert_eq!(value["sub_model"], json!("videotape-alpha"));
    assert_eq!(value["aspect_ratio"], json!("1:1"));
    assert_eq!(value["batch_size"], json!(2));
  }

  // ── Validation ──

  #[test]
  fn empty_prompt_is_rejected() {
    let request = GptImage2Request::text_to_image("", GptImage2AspectRatio::Square1x1, GptImage2Quality::Low, GptImage2Resolution::OneK);
    assert!(matches!(request.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[tokio::test]
  async fn invalid_request_fails_before_any_http() {
    let auth = HiggsfieldAuth::new("token");
    let host = HiggsfieldHost::Custom("http://127.0.0.1:9".to_string());
    let request = GptImage2Request::text_to_image(" ", GptImage2AspectRatio::Square1x1, GptImage2Quality::Low, GptImage2Resolution::OneK);

    let err = gpt_image_2(GptImage2Args { request, auth: &auth, host: &host }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  // ── Inbound shape ──

  #[test]
  fn enqueue_response_parses() {
    use crate::types::enqueue_jobs_response::tests::GPT_IMAGE_ENQUEUE_RESPONSE;
    use crate::types::job_set_type::JobSetType;
    let response: EnqueueJobsResponse = serde_json::from_str(GPT_IMAGE_ENQUEUE_RESPONSE).unwrap();
    assert_eq!(response.first_job_set().unwrap().job_set_type, JobSetType::GptImage2);
  }

  // ── Live (ignored: needs a real session and spends credits) ──

  #[tokio::test]
  #[ignore]
  async fn live_enqueue_gpt_image_2() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_auth;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let auth = load_higgsfield_test_auth().await?;
    let response = gpt_image_2(GptImage2Args {
      request: GptImage2Request::text_to_image("a corgi on a bike", GptImage2AspectRatio::Portrait9x16, GptImage2Quality::High, GptImage2Resolution::TwoK),
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    println!("Enqueued job ids: {:?}", response.job_ids());
    println!("Wallet: {:?}", response.wallet);
    assert!(!response.job_ids().is_empty());
    Ok(())
  }

  /// Enqueues a GPT Image 2 job off the desktop app's saved Higgsfield
  /// login (`~/Artcraft/artcraftx/credentials/higgsfield_cookies.toml`),
  /// prints the enqueue response, then follows the job through the status
  /// endpoints until it completes. Cheapest settings (1 image, 1k, low).
  #[tokio::test]
  #[ignore]
  async fn live_enqueue_gpt_image_2_from_app_credential_and_poll() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::poll_job_to_completion::poll_job_to_completion;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("minting a session token failed: {err}"))?;

    let request = GptImage2Request::text_to_image("a corgi on a bike", GptImage2AspectRatio::Square1x1, GptImage2Quality::Low, GptImage2Resolution::OneK);
    println!("\n===== request =====\n{:#?}", request);

    let response = gpt_image_2(GptImage2Args {
      request,
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    println!("\n===== POST /fnf/jobs/v2/gpt_image_2 =====\n{:#?}", response);
    let job_ids = response.job_ids();
    println!("job ids: {:?}", job_ids);
    assert_eq!(job_ids.len(), 1);
    assert_eq!(response.first_job_set().unwrap().job_set_type, crate::types::job_set_type::JobSetType::GptImage2);

    let job = poll_job_to_completion(&session, &job_ids[0]).await?;
    assert!(job.result_url().is_some(), "completed job should have a result url");
    Ok(())
  }
}
