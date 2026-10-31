//! POST `/fnf/jobs/nano-banana-2` — enqueue a Nano Banana Pro image job.
//!
//! "Nano Banana Pro" is the web app's name for the pipeline the API calls
//! `nano-banana-2` (job set type `nano_banana_2`).
//!
//! Option sets below were read off the web app's image generator on
//! 2026-08-31: 11 aspect ratios (incl. Auto), 1K/2K/4K, 1–4 images.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::enqueue_jobs_response::EnqueueJobsResponse;
use crate::types::image_batch_size::ImageBatchSize;
use crate::types::image_dimensions::ImageDimensions;
use crate::types::image_resolution::ImageResolution;
use crate::types::media_input::MediaInput;
use crate::types::nano_banana_aspect_ratio::NanoBananaAspectRatio;
use serde::Serialize;

const PATH: &str = "/fnf/jobs/nano-banana-2";

/// Nano Banana Pro offers the same aspect menu as the other Nano Banana
/// models.
pub type NanoBananaProAspectRatio = NanoBananaAspectRatio;

/// The resolution tiers the web app offers for Nano Banana Pro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum NanoBananaProResolution {
  #[default]
  OneK,
  TwoK,
  FourK,
}

impl NanoBananaProResolution {
  pub fn all() -> [Self; 3] {
    [Self::OneK, Self::TwoK, Self::FourK]
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

impl Serialize for NanoBananaProResolution {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.as_str())
  }
}

pub struct NanoBananaProArgs<'a> {
  pub request: NanoBananaProRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// The material part of a Nano Banana Pro request. Serializable so it can be
/// logged or persisted separately from the session.
#[derive(Clone, Debug, Serialize)]
pub struct NanoBananaProRequest {
  pub prompt: String,

  pub aspect_ratio: NanoBananaProAspectRatio,

  pub resolution: NanoBananaProResolution,

  /// How many images to generate (1–4). Each costs credits.
  pub batch_size: ImageBatchSize,

  /// Reference images (image-to-image), uploaded first via
  /// `endpoints::media` / `HiggsfieldSession::upload_reference_media`.
  /// Empty for text-to-image. Sent as `input_images` (this endpoint
  /// predates the role-tagged `medias` list). One reference raised the
  /// price from 150 to 200 credits at 1k.
  pub reference_images: Vec<MediaInput>,

  /// Spend from the plan's "unlimited" pool instead of credits, if the plan
  /// has one.
  pub use_unlim: bool,

  /// Override the pixel size sent with the request. When `None`, the size
  /// the web app would send for `aspect_ratio` + `resolution` is used.
  pub maybe_dimensions: Option<ImageDimensions>,
}

impl NanoBananaProRequest {
  /// A text-to-image request with the web app's defaults (1 image, credits).
  pub fn text_to_image(prompt: impl Into<String>, aspect_ratio: NanoBananaProAspectRatio, resolution: NanoBananaProResolution) -> Self {
    Self {
      prompt: prompt.into(),
      aspect_ratio,
      resolution,
      batch_size: ImageBatchSize::One,
      reference_images: Vec::new(),
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

  fn to_body(&self) -> Result<NanoBananaProRequestBody, HiggsfieldClientError> {
    let dimensions = self.dimensions()?;
    Ok(NanoBananaProRequestBody {
      params: NanoBananaProParams {
        prompt: self.prompt.clone(),
        aspect_ratio: self.aspect_ratio,
        resolution: self.resolution,
        batch_size: self.batch_size,
        is_storyboard: false,
        is_zoom_control: false,
        use_unlim: self.use_unlim,
        width: dimensions.width,
        height: dimensions.height,
        input_images: self.reference_images.clone(),
      },
      use_unlim: self.use_unlim,
    })
  }
}

/// Enqueue the job. The response's job ids are what to poll (see
/// `endpoints::jobs`).
pub async fn nano_banana_pro(args: NanoBananaProArgs<'_>) -> Result<EnqueueJobsResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body()?;
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct NanoBananaProRequestBody {
  params: NanoBananaProParams,
  use_unlim: bool,
}

#[derive(Serialize)]
struct NanoBananaProParams {
  prompt: String,
  aspect_ratio: NanoBananaProAspectRatio,
  resolution: NanoBananaProResolution,
  batch_size: ImageBatchSize,
  is_storyboard: bool,
  is_zoom_control: bool,
  use_unlim: bool,
  width: u32,
  height: u32,
  input_images: Vec<MediaInput>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::media_input::MediaInput;
  use serde_json::{json, Value};

  // ── Option sets ──

  #[test]
  fn aspect_ratios_match_the_web_app_menu() {
    let wire: Vec<&str> = NanoBananaProAspectRatio::all().iter().map(|a| a.as_str()).collect();
    assert_eq!(wire, ["auto", "1:1", "3:4", "4:3", "2:3", "3:2", "9:16", "16:9", "5:4", "4:5", "21:9"]);
  }

  #[test]
  fn resolutions_match_the_web_app_menu() {
    let wire: Vec<&str> = NanoBananaProResolution::all().iter().map(|r| r.as_str()).collect();
    assert_eq!(wire, ["1k", "2k", "4k"]);
    assert_eq!(NanoBananaProResolution::default(), NanoBananaProResolution::OneK);
  }

  // ── Outbound shape ──

  #[test]
  fn wire_body_matches_captured_request() {
    // Captured from the web app: 3:4 at 1k, one image.
    let request = NanoBananaProRequest::text_to_image("a dinosaur on a skateboard", NanoBananaProAspectRatio::Portrait3x4, NanoBananaProResolution::OneK);
    let actual: Value = serde_json::to_value(request.to_body().unwrap()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a dinosaur on a skateboard","aspect_ratio":"3:4","resolution":"1k","batch_size":1,"is_storyboard":false,"is_zoom_control":false,"use_unlim":false,"width":896,"height":1200,"input_images":[]},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn wire_body_with_reference_image_matches_captured_request() {
    // Captured from the web app 2026-08-31 (ids scrubbed): one uploaded
    // reference goes out as a bare descriptor in `input_images`. The web app
    // sent 1376x768 for 16:9@1k; this client's derivation is asserted.
    let request = NanoBananaProRequest::text_to_image("a corgi on a bike", NanoBananaProAspectRatio::Landscape16x9, NanoBananaProResolution::OneK)
        .with_reference_images(vec![MediaInput::uploaded("00000000-0000-4000-8000-0000000000aa", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png")]);
    let actual: Value = serde_json::to_value(request.to_body().unwrap()).unwrap();
    let dimensions = request.dimensions().unwrap();
    let expected_json = r#"{"params":{"prompt":"a corgi on a bike","input_images":[{"id":"00000000-0000-4000-8000-0000000000aa","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png"}],"aspect_ratio":"16:9","resolution":"1k","batch_size":1,"is_storyboard":false,"is_zoom_control":false,"use_unlim":false,"width":W,"height":H},"use_unlim":false}"#
        .replace("\"width\":W", &format!("\"width\":{}", dimensions.width)).replace("\"height\":H", &format!("\"height\":{}", dimensions.height));
    let expected: Value = serde_json::from_str(&expected_json).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn wire_body_4k_landscape_matches_captured_request() {
    let request = NanoBananaProRequest::text_to_image("a dinosaur on a skateboard", NanoBananaProAspectRatio::Landscape16x9, NanoBananaProResolution::FourK);
    let dimensions = request.dimensions().unwrap();
    assert_eq!((dimensions.width, dimensions.height), (5504, 3072));
  }

  #[test]
  fn every_option_combination_derives_dimensions() {
    for ratio in NanoBananaProAspectRatio::all() {
      for resolution in NanoBananaProResolution::all() {
        let request = NanoBananaProRequest::text_to_image("p", ratio, resolution);
        request.dimensions().unwrap_or_else(|err| panic!("{} @ {}: {err}", ratio.as_str(), resolution.as_str()));
      }
    }
  }

  #[test]
  fn explicit_dimensions_override_derived_ones() {
    let mut request = NanoBananaProRequest::text_to_image("p", NanoBananaProAspectRatio::Square1x1, NanoBananaProResolution::OneK);
    request.maybe_dimensions = Some(ImageDimensions::new(1024, 1024));
    assert_eq!(request.dimensions().unwrap(), ImageDimensions::new(1024, 1024));
  }

  #[test]
  fn public_request_serializes_with_typed_enums() {
    let mut request = NanoBananaProRequest::text_to_image("a cat", NanoBananaProAspectRatio::Landscape16x9, NanoBananaProResolution::TwoK);
    request.batch_size = ImageBatchSize::Four;
    let value = serde_json::to_value(&request).unwrap();
    assert_eq!(value["aspect_ratio"], json!("16:9"));
    assert_eq!(value["resolution"], json!("2k"));
    assert_eq!(value["batch_size"], json!(4));
  }

  // ── Validation ──

  #[test]
  fn empty_prompt_is_rejected() {
    let request = NanoBananaProRequest::text_to_image("   ", NanoBananaProAspectRatio::Square1x1, NanoBananaProResolution::OneK);
    assert!(matches!(request.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[tokio::test]
  async fn invalid_request_fails_before_any_http() {
    // No network: an unroutable custom host would fail differently.
    let auth = HiggsfieldAuth::new("token");
    let host = HiggsfieldHost::Custom("http://127.0.0.1:9".to_string());
    let request = NanoBananaProRequest::text_to_image("", NanoBananaProAspectRatio::Square1x1, NanoBananaProResolution::OneK);

    let err = nano_banana_pro(NanoBananaProArgs { request, auth: &auth, host: &host }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  // ── Inbound shape ──

  #[test]
  fn enqueue_response_parses() {
    use crate::types::enqueue_jobs_response::tests::NANO_BANANA_ENQUEUE_RESPONSE;
    let response: EnqueueJobsResponse = serde_json::from_str(NANO_BANANA_ENQUEUE_RESPONSE).unwrap();
    assert_eq!(response.job_ids().len(), 1);
  }

  // ── Live (ignored: needs a real session and spends credits) ──

  #[tokio::test]
  #[ignore]
  async fn live_enqueue_nano_banana_pro() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_auth;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let auth = load_higgsfield_test_auth().await?;
    let response = nano_banana_pro(NanoBananaProArgs {
      request: NanoBananaProRequest::text_to_image("a dinosaur on a skateboard", NanoBananaProAspectRatio::Portrait3x4, NanoBananaProResolution::OneK),
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    println!("Enqueued job ids: {:?}", response.job_ids());
    println!("Wallet: {:?}", response.wallet);
    assert!(!response.job_ids().is_empty());
    Ok(())
  }

  /// Enqueues a Nano Banana Pro job off the desktop app's saved Higgsfield
  /// login (`~/Artcraft/artcraftx/credentials/higgsfield_cookies.toml`),
  /// prints the enqueue response, then follows the job through the status
  /// endpoints until it completes. Cheapest settings (1 image, 1k).
  #[tokio::test]
  #[ignore]
  async fn live_enqueue_nano_banana_pro_from_app_credential_and_poll() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::poll_job_to_completion::poll_job_to_completion;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("minting a session token failed: {err}"))?;

    let request = NanoBananaProRequest::text_to_image("a dinosaur on a skateboard", NanoBananaProAspectRatio::Portrait3x4, NanoBananaProResolution::OneK);
    println!("\n===== request =====\n{:#?}", request);

    let response = nano_banana_pro(NanoBananaProArgs {
      request,
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    println!("\n===== POST /fnf/jobs/nano-banana-2 =====\n{:#?}", response);
    let job_ids = response.job_ids();
    println!("job ids: {:?}", job_ids);
    assert_eq!(job_ids.len(), 1);
    assert_eq!(response.first_job_set().unwrap().job_set_type, crate::types::job_set_type::JobSetType::NanoBanana2);

    let job = poll_job_to_completion(&session, &job_ids[0]).await?;
    assert!(job.result_url().is_some(), "completed job should have a result url");
    Ok(())
  }
}
