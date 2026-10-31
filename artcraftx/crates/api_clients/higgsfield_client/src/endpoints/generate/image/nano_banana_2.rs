//! POST `/fnf/jobs/v2/nano_banana_flash` — enqueue a Nano Banana 2 image
//! job. "Nano Banana 2" is the web app's name for the pipeline the API
//! calls `nano_banana_flash` (job set type `nano_banana_flash`).
//!
//! Option sets below were read off the web app's image generator on
//! 2026-08-31: 11 aspect ratios (incl. Auto), 1K / 2K / 4K, 1–4 images.

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
use serde::Serialize;

const PATH: &str = "/fnf/jobs/v2/nano_banana_flash";

/// The resolution tiers the web app offers for Nano Banana 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum NanoBanana2Resolution {
  #[default]
  OneK,
  TwoK,
  FourK,
}

impl NanoBanana2Resolution {
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

impl Serialize for NanoBanana2Resolution {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.as_str())
  }
}

pub struct NanoBanana2Args<'a> {
  pub request: NanoBanana2Request,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// The material part of a Nano Banana 2 request. Serializable so it can be
/// logged or persisted separately from the session.
#[derive(Clone, Debug, Serialize)]
pub struct NanoBanana2Request {
  pub prompt: String,

  pub aspect_ratio: NanoBananaAspectRatio,

  pub resolution: NanoBanana2Resolution,

  /// How many images to generate (1–4). Each costs credits.
  pub batch_size: ImageBatchSize,

  /// Reference images (image-to-image), uploaded first via
  /// `endpoints::media` / `HiggsfieldSession::upload_reference_media`.
  /// Empty for text-to-image (then `medias` is omitted, as the web app
  /// does). Sent as `medias` with role `image`.
  pub reference_images: Vec<MediaInput>,

  /// Spend from the plan's "unlimited" pool instead of credits, if the plan
  /// has one (this model is offered on it).
  pub use_unlim: bool,

  /// Override the pixel size sent with the request. When `None`, the size
  /// the web app would send for `aspect_ratio` + `resolution` is used.
  pub maybe_dimensions: Option<ImageDimensions>,
}

impl NanoBanana2Request {
  /// A text-to-image request with the web app's defaults (1 image, credits).
  pub fn text_to_image(prompt: impl Into<String>, aspect_ratio: NanoBananaAspectRatio, resolution: NanoBanana2Resolution) -> Self {
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

  fn to_body(&self) -> Result<NanoBanana2RequestBody, HiggsfieldClientError> {
    let dimensions = self.dimensions()?;
    Ok(NanoBanana2RequestBody {
      params: NanoBanana2Params {
        prompt: self.prompt.clone(),
        medias: self.reference_images.iter().cloned().map(MediaReference::image).collect(),
        aspect_ratio: self.aspect_ratio,
        resolution: self.resolution,
        batch_size: self.batch_size,
        use_unlim: self.use_unlim,
        width: dimensions.width,
        height: dimensions.height,
      },
      use_unlim: self.use_unlim,
    })
  }
}

/// Enqueue the job. The response's job ids are what to poll (see
/// `endpoints::jobs`).
pub async fn nano_banana_2(args: NanoBanana2Args<'_>) -> Result<EnqueueJobsResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body()?;
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct NanoBanana2RequestBody {
  params: NanoBanana2Params,
  use_unlim: bool,
}

#[derive(Serialize)]
struct NanoBanana2Params {
  prompt: String,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  medias: Vec<MediaReference>,
  aspect_ratio: NanoBananaAspectRatio,
  resolution: NanoBanana2Resolution,
  batch_size: ImageBatchSize,
  use_unlim: bool,
  width: u32,
  height: u32,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::media_input::MediaInput;
  use crate::types::job_set_type::JobSetType;
  use serde_json::Value;

  /// Captured enqueue response, ids / user scrubbed.
  const ENQUEUE_RESPONSE: &str = r#"{"id":"00000000-0000-0000-0000-00000000aaaa","job_sets":[{"id":"00000000-0000-0000-0000-0000000068dc","type":"nano_banana_flash","project_id":"00000000-0000-0000-0000-00000000aaaa","created_at":1788154942.202193,"parent_id":null,"cluster_hash":"d8cd56aedaaaeedac3a5ed262f5eed0e","cost":150.0,"params":{"width":864,"height":1184,"aspect_ratio":"3:4","resolution":"1k","batch_size":1,"is_inpaint":false,"prompt":"a corgi on a bike","medias":[],"reference_elements":[]},"jobs":[{"id":"00000000-0000-0000-0000-0000a7ee94f9","status":"queued","ip_check_finished":null,"ip_detected":null,"result":null,"results":null,"board_ids":[],"published_at":null,"meta":{},"created_at":1788154942.210965,"user_id":"user_TESTUSER0000000000000000000","trace_id":"00000000-0000-0000-0000-0000a7ee94f9","cluster_hash":"d8cd56aedaaaeedac3a5ed262f5eed0e","representation":null,"folder_ids":[],"is_favourite":false}],"client_meta":null,"chain_id":null}],"has_more":false,"wallet":{"workspace_id":"00000000-0000-0000-0000-00000000aaaa","credits_balance":0,"subscription_balance":118600,"wallet_created_at":"2026-08-31T03:26:11.027426Z","next_credit_allocation_date":null,"total_credits":120000,"on_demand_credits":0,"expire_days":90},"workspace_details":{"id":"00000000-0000-0000-0000-00000000aaaa","name":null,"type":"private","user_role":"owner","clerk_organization_id":null,"avatar_url":null,"bio":null,"grace_period_type":null,"sso_status":"idle","is_enterprise_sub_workspace":false,"sub_workspace_block":null},"free_gens_v2":{"items":[]},"generation_seconds":{"items":[]},"folder_credits":null}"#;

  #[test]
  fn resolutions_match_the_web_app_menu() {
    let wire: Vec<&str> = NanoBanana2Resolution::all().iter().map(|r| r.as_str()).collect();
    assert_eq!(wire, ["1k", "2k", "4k"]);
  }

  #[test]
  fn wire_body_matches_captured_request() {
    let request = NanoBanana2Request::text_to_image("a corgi on a bike", NanoBananaAspectRatio::Portrait3x4, NanoBanana2Resolution::OneK);
    let actual: Value = serde_json::to_value(request.to_body().unwrap()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"3:4","resolution":"1k","batch_size":1,"use_unlim":false,"width":896,"height":1200},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn every_option_combination_derives_dimensions() {
    for ratio in NanoBananaAspectRatio::all() {
      for resolution in NanoBanana2Resolution::all() {
        NanoBanana2Request::text_to_image("p", ratio, resolution).dimensions()
            .unwrap_or_else(|err| panic!("{} @ {}: {err}", ratio.as_str(), resolution.as_str()));
      }
    }
  }

  #[test]
  fn wire_body_with_reference_image_matches_captured_request() {
    // Captured from the web app 2026-08-31 (ids scrubbed).
    let request = NanoBanana2Request::text_to_image("a corgi on a bike", NanoBananaAspectRatio::Portrait3x4, NanoBanana2Resolution::OneK)
        .with_reference_images(vec![MediaInput::uploaded("00000000-0000-4000-8000-0000000000aa", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png")]);
    let actual: Value = serde_json::to_value(request.to_body().unwrap()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a corgi on a bike","medias":[{"role":"image","data":{"id":"00000000-0000-4000-8000-0000000000aa","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png"}}],"aspect_ratio":"3:4","resolution":"1k","batch_size":1,"use_unlim":false,"width":896,"height":1200},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn empty_prompt_is_rejected() {
    let request = NanoBanana2Request::text_to_image(" ", NanoBananaAspectRatio::Square1x1, NanoBanana2Resolution::OneK);
    assert!(matches!(request.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[tokio::test]
  async fn invalid_request_fails_before_any_http() {
    let auth = HiggsfieldAuth::new("token");
    let host = HiggsfieldHost::Custom("http://127.0.0.1:9".to_string());
    let request = NanoBanana2Request::text_to_image("", NanoBananaAspectRatio::Square1x1, NanoBanana2Resolution::OneK);
    let err = nano_banana_2(NanoBanana2Args { request, auth: &auth, host: &host }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn enqueue_response_parses() {
    let response: EnqueueJobsResponse = serde_json::from_str(ENQUEUE_RESPONSE).unwrap();
    let job_set = response.first_job_set().unwrap();
    assert_eq!(job_set.job_set_type, JobSetType::NanoBananaFlash);
    assert_eq!(job_set.cost, Some(150.0));
    assert_eq!(job_set.params.resolution, Some(ImageResolution::OneK));
  }

  // ── Live (ignored: needs a real session and spends credits) ──

  #[tokio::test]
  #[ignore]
  async fn live_enqueue_nano_banana_2_from_app_credential_and_poll() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::poll_job_to_completion::poll_job_to_completion;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    let request = NanoBanana2Request::text_to_image("a corgi on a bike", NanoBananaAspectRatio::Portrait3x4, NanoBanana2Resolution::OneK);
    println!("\n===== request =====\n{:#?}", request);

    let response = nano_banana_2(NanoBanana2Args { request, auth: &auth, host: &HiggsfieldHost::Higgsfield })
        .await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("\n===== POST {PATH} =====\n{:#?}", response);
    let job_ids = response.job_ids();
    assert_eq!(response.first_job_set().unwrap().job_set_type, JobSetType::NanoBananaFlash);

    let job = poll_job_to_completion(&session, &job_ids[0]).await?;
    assert!(job.result_url().is_some());
    Ok(())
  }
}
