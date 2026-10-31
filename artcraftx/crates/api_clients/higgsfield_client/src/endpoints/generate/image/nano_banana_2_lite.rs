//! POST `/fnf/jobs/v2/nano_banana_2_lite` — enqueue a Nano Banana 2 Lite
//! image job (job set type `nano_banana_2_lite`).
//!
//! Option sets below were read off the web app's image generator on
//! 2026-08-31: 11 aspect ratios (incl. Auto), a High / Minimal quality menu
//! sent as `thinking`, 1–4 images. There is no resolution menu — the app
//! always sends `resolution: "1k"`.

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
use crate::types::media_reference::MediaReference;
use crate::types::nano_banana_aspect_ratio::NanoBananaAspectRatio;
use crate::types::thinking_level::ThinkingLevel;
use serde::Serialize;

const PATH: &str = "/fnf/jobs/v2/nano_banana_2_lite";

/// The only resolution this pipeline runs at.
const RESOLUTION: ImageResolution = ImageResolution::OneK;

/// The web app's quality menu for Nano Banana 2 Lite ("High" / "Minimal"),
/// sent as the model's `thinking` level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum NanoBanana2LiteQuality {
  #[default]
  High,
  Minimal,
}

impl NanoBanana2LiteQuality {
  pub fn all() -> [Self; 2] {
    [Self::High, Self::Minimal]
  }

  pub fn to_thinking_level(self) -> ThinkingLevel {
    match self {
      Self::High => ThinkingLevel::High,
      Self::Minimal => ThinkingLevel::Minimal,
    }
  }
}

/// Serializes as the wire `thinking` value.
impl Serialize for NanoBanana2LiteQuality {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.to_thinking_level().as_str())
  }
}

pub struct NanoBanana2LiteArgs<'a> {
  pub request: NanoBanana2LiteRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// The material part of a Nano Banana 2 Lite request. Serializable so it
/// can be logged or persisted separately from the session.
#[derive(Clone, Debug, Serialize)]
pub struct NanoBanana2LiteRequest {
  pub prompt: String,

  pub aspect_ratio: NanoBananaAspectRatio,

  pub quality: NanoBanana2LiteQuality,

  /// How many images to generate (1–4). Each costs credits.
  pub batch_size: ImageBatchSize,

  /// Reference images (image-to-image), uploaded first via
  /// `endpoints::media` / `HiggsfieldSession::upload_reference_media`.
  /// Empty for text-to-image (then `medias` is omitted, as the web app
  /// does). Sent as `medias` with role `image`.
  pub reference_images: Vec<MediaInput>,

  /// Spend from the plan's "unlimited" pool instead of credits, if the plan
  /// has one.
  pub use_unlim: bool,

  /// Override the pixel size sent with the request. When `None`, the size
  /// the web app would send for `aspect_ratio` at 1k is used.
  pub maybe_dimensions: Option<ImageDimensions>,
}

impl NanoBanana2LiteRequest {
  /// A text-to-image request with the web app's defaults (1 image, credits).
  pub fn text_to_image(prompt: impl Into<String>, aspect_ratio: NanoBananaAspectRatio, quality: NanoBanana2LiteQuality) -> Self {
    Self {
      prompt: prompt.into(),
      aspect_ratio,
      quality,
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
    ImageDimensions::for_aspect_ratio(&self.aspect_ratio.to_image_aspect_ratio(), &RESOLUTION)
        .ok_or_else(|| HiggsfieldClientError::InvalidRequest(format!(
          "can't derive dimensions for aspect ratio {}; pass maybe_dimensions", self.aspect_ratio.as_str(),
        )))
  }

  fn to_body(&self) -> Result<NanoBanana2LiteRequestBody, HiggsfieldClientError> {
    let dimensions = self.dimensions()?;
    Ok(NanoBanana2LiteRequestBody {
      params: NanoBanana2LiteParams {
        prompt: self.prompt.clone(),
        medias: self.reference_images.iter().cloned().map(MediaReference::image).collect(),
        aspect_ratio: self.aspect_ratio,
        resolution: RESOLUTION,
        thinking: self.quality.to_thinking_level(),
        batch_size: self.batch_size,
        width: dimensions.width,
        height: dimensions.height,
      },
      use_unlim: self.use_unlim,
    })
  }
}

/// Enqueue the job. The response's job ids are what to poll (see
/// `endpoints::jobs`).
pub async fn nano_banana_2_lite(args: NanoBanana2LiteArgs<'_>) -> Result<EnqueueJobsResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body()?;
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct NanoBanana2LiteRequestBody {
  params: NanoBanana2LiteParams,
  use_unlim: bool,
}

/// NB: unlike the other Nano Banana endpoints, the web app does NOT put
/// `use_unlim` inside `params` here.
#[derive(Serialize)]
struct NanoBanana2LiteParams {
  prompt: String,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  medias: Vec<MediaReference>,
  aspect_ratio: NanoBananaAspectRatio,
  resolution: ImageResolution,
  thinking: ThinkingLevel,
  batch_size: ImageBatchSize,
  width: u32,
  height: u32,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::media_input::MediaInput;
  use crate::types::job_set_type::JobSetType;
  use serde_json::Value;

  /// Captured enqueue response (MINIMAL), ids / user scrubbed.
  const ENQUEUE_RESPONSE: &str = r#"{"id":"00000000-0000-0000-0000-00000000aaaa","job_sets":[{"id":"00000000-0000-0000-0000-00000000943d","type":"nano_banana_2_lite","project_id":"00000000-0000-0000-0000-00000000aaaa","created_at":1788155026.294231,"parent_id":null,"cluster_hash":"d8cd56aedaaaeedac3a5ed262f5eed0e","cost":100,"params":{"width":864,"height":1184,"aspect_ratio":"3:4","batch_size":1,"thinking":"MINIMAL","is_inpaint":false,"prompt":"a corgi on a bike","medias":[],"reference_elements":[]},"jobs":[{"id":"00000000-0000-0000-0000-00005900ddea","status":"queued","ip_check_finished":null,"ip_detected":null,"result":null,"results":null,"board_ids":[],"published_at":null,"meta":{},"created_at":1788155026.303344,"user_id":"user_TESTUSER0000000000000000000","trace_id":"00000000-0000-0000-0000-00005900ddea","cluster_hash":"d8cd56aedaaaeedac3a5ed262f5eed0e","representation":null,"folder_ids":[],"is_favourite":false}],"client_meta":null,"chain_id":null}],"has_more":false,"wallet":{"workspace_id":"00000000-0000-0000-0000-00000000aaaa","credits_balance":0,"subscription_balance":118500,"wallet_created_at":"2026-08-31T03:26:11.027426Z","next_credit_allocation_date":null,"total_credits":120000,"on_demand_credits":0,"expire_days":90},"workspace_details":{"id":"00000000-0000-0000-0000-00000000aaaa","name":null,"type":"private","user_role":"owner","clerk_organization_id":null,"avatar_url":null,"bio":null,"grace_period_type":null,"sso_status":"idle","is_enterprise_sub_workspace":false,"sub_workspace_block":null},"free_gens_v2":{"items":[]},"generation_seconds":{"items":[]},"folder_credits":null}"#;

  #[test]
  fn quality_menu_maps_to_thinking() {
    let mapping: Vec<String> = NanoBanana2LiteQuality::all().iter().map(|q| q.to_thinking_level().to_string()).collect();
    assert_eq!(mapping, ["HIGH", "MINIMAL"]);
  }

  #[test]
  fn wire_bodies_match_captured_requests() {
    let cases = [
      (NanoBanana2LiteQuality::Minimal, r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"3:4","resolution":"1k","thinking":"MINIMAL","batch_size":1,"width":896,"height":1200},"use_unlim":false}"#),
      (NanoBanana2LiteQuality::High, r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"3:4","resolution":"1k","thinking":"HIGH","batch_size":1,"width":896,"height":1200},"use_unlim":false}"#),
    ];
    for (quality, expected) in cases {
      let request = NanoBanana2LiteRequest::text_to_image("a corgi on a bike", NanoBananaAspectRatio::Portrait3x4, quality);
      let actual: Value = serde_json::to_value(request.to_body().unwrap()).unwrap();
      let expected: Value = serde_json::from_str(expected).unwrap();
      assert_eq!(actual, expected, "{:?}", quality);
    }
  }

  #[test]
  fn every_aspect_derives_dimensions() {
    for ratio in NanoBananaAspectRatio::all() {
      NanoBanana2LiteRequest::text_to_image("p", ratio, NanoBanana2LiteQuality::High).dimensions()
          .unwrap_or_else(|err| panic!("{}: {err}", ratio.as_str()));
    }
  }

  #[test]
  fn wire_body_with_reference_image_matches_captured_request() {
    // Captured from the web app 2026-08-31 (ids scrubbed).
    let request = NanoBanana2LiteRequest::text_to_image("a corgi on a bike", NanoBananaAspectRatio::Portrait3x4, NanoBanana2LiteQuality::High)
        .with_reference_images(vec![MediaInput::uploaded("00000000-0000-4000-8000-0000000000aa", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png")]);
    let actual: Value = serde_json::to_value(request.to_body().unwrap()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a corgi on a bike","medias":[{"role":"image","data":{"id":"00000000-0000-4000-8000-0000000000aa","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png"}}],"aspect_ratio":"3:4","resolution":"1k","thinking":"HIGH","batch_size":1,"width":896,"height":1200},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn empty_prompt_is_rejected() {
    let request = NanoBanana2LiteRequest::text_to_image("", NanoBananaAspectRatio::Square1x1, NanoBanana2LiteQuality::High);
    assert!(matches!(request.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[tokio::test]
  async fn invalid_request_fails_before_any_http() {
    let auth = HiggsfieldAuth::new("token");
    let host = HiggsfieldHost::Custom("http://127.0.0.1:9".to_string());
    let request = NanoBanana2LiteRequest::text_to_image("", NanoBananaAspectRatio::Square1x1, NanoBanana2LiteQuality::High);
    let err = nano_banana_2_lite(NanoBanana2LiteArgs { request, auth: &auth, host: &host }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn enqueue_response_parses() {
    let response: EnqueueJobsResponse = serde_json::from_str(ENQUEUE_RESPONSE).unwrap();
    let job_set = response.first_job_set().unwrap();
    assert_eq!(job_set.job_set_type, JobSetType::NanoBanana2Lite);
    assert_eq!(job_set.params.thinking, Some(ThinkingLevel::Minimal));
    assert!(job_set.params.resolution.is_none(), "the server drops resolution from the echoed params");
  }

  // ── Live (ignored: needs a real session and spends credits) ──

  #[tokio::test]
  #[ignore]
  async fn live_enqueue_nano_banana_2_lite_from_app_credential_and_poll() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::poll_job_to_completion::poll_job_to_completion;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    let request = NanoBanana2LiteRequest::text_to_image("a corgi on a bike", NanoBananaAspectRatio::Portrait3x4, NanoBanana2LiteQuality::Minimal);
    println!("\n===== request =====\n{:#?}", request);

    let response = nano_banana_2_lite(NanoBanana2LiteArgs { request, auth: &auth, host: &HiggsfieldHost::Higgsfield })
        .await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("\n===== POST {PATH} =====\n{:#?}", response);
    let job_ids = response.job_ids();
    assert_eq!(response.first_job_set().unwrap().job_set_type, JobSetType::NanoBanana2Lite);

    let job = poll_job_to_completion(&session, &job_ids[0]).await?;
    assert!(job.result_url().is_some());
    Ok(())
  }
}
