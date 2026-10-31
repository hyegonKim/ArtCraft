//! POST `/fnf/jobs/v2/seedream_v5_lite` — enqueue a Seedream 5.0 lite
//! image job (job set type `seedream_v5_lite`).
//!
//! Option sets below were read off the web app's image generator on
//! 2026-08-31: 8 aspect ratios, a 2K / 3K / 4K menu that the app sends as
//! `quality` basic / high / ultra, 1–4 images. The app always sends
//! `width`/`height` 1024×1024 with it; the server derives the real size.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::enqueue_jobs_response::EnqueueJobsResponse;
use crate::types::image_batch_size::ImageBatchSize;
use crate::types::image_quality::ImageQuality;
use crate::types::image_seed::ImageSeed;
use crate::types::media_input::MediaInput;
use crate::types::media_reference::MediaReference;
use crate::types::seedream_aspect_ratio::SeedreamAspectRatio;
use serde::Serialize;

const PATH: &str = "/fnf/jobs/v2/seedream_v5_lite";

/// The `model` field the web app sends on this endpoint.
const MODEL: &str = "seedream_v5_lite";

/// The placeholder size the web app always sends; the server ignores it.
const PLACEHOLDER_WIDTH: u32 = 1024;
const PLACEHOLDER_HEIGHT: u32 = 1024;

/// The web app's resolution menu for Seedream 5.0 lite. It goes out as the
/// `quality` param.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Seedream5p0LiteResolution {
  /// 2K → `basic`
  #[default]
  TwoK,
  /// 3K → `high`
  ThreeK,
  /// 4K → `ultra`
  FourK,
}

impl Seedream5p0LiteResolution {
  pub fn all() -> [Self; 3] {
    [Self::TwoK, Self::ThreeK, Self::FourK]
  }

  /// The menu label.
  pub fn label(self) -> &'static str {
    match self {
      Self::TwoK => "2K",
      Self::ThreeK => "3K",
      Self::FourK => "4K",
    }
  }

  /// The `quality` the web app sends for this tier.
  pub fn to_image_quality(self) -> ImageQuality {
    match self {
      Self::TwoK => ImageQuality::Basic,
      Self::ThreeK => ImageQuality::High,
      Self::FourK => ImageQuality::Ultra,
    }
  }
}

/// Serializes as the menu label ("2K" / "3K" / "4K") so a logged request
/// reads the way the user chose it; the wire `quality` is derived at send.
impl Serialize for Seedream5p0LiteResolution {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.label())
  }
}

pub struct Seedream5p0LiteArgs<'a> {
  pub request: Seedream5p0LiteRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// The material part of a Seedream 5.0 lite request. Serializable so it can
/// be logged or persisted separately from the session.
#[derive(Clone, Debug, Serialize)]
pub struct Seedream5p0LiteRequest {
  pub prompt: String,

  pub aspect_ratio: SeedreamAspectRatio,

  pub resolution: Seedream5p0LiteResolution,

  /// How many images to generate (1–4). Each costs credits.
  pub batch_size: ImageBatchSize,

  /// Reference images (image-to-image), uploaded first via
  /// `endpoints::media` / `HiggsfieldSession::upload_reference_media`.
  /// Empty for text-to-image (then `medias` is omitted, as the web app
  /// does). Sent as `medias` with role `image`.
  ///
  /// NB: with a reference the web app sends the reference's pixel size as
  /// `width`/`height` instead of the placeholder; the server derives the
  /// real output size from `quality` either way, so this client keeps the
  /// placeholder.
  pub reference_images: Vec<MediaInput>,

  /// Pin the generation seed; `None` sends a fresh random one, as the web
  /// app does.
  pub maybe_seed: Option<ImageSeed>,

  /// Spend from the plan's "unlimited" pool instead of credits, if the plan
  /// has one (this model is offered on it).
  pub use_unlim: bool,
}

impl Seedream5p0LiteRequest {
  /// A text-to-image request with the web app's defaults (1 image, random
  /// seed, credits).
  pub fn text_to_image(prompt: impl Into<String>, aspect_ratio: SeedreamAspectRatio, resolution: Seedream5p0LiteResolution) -> Self {
    Self {
      prompt: prompt.into(),
      aspect_ratio,
      resolution,
      batch_size: ImageBatchSize::One,
      reference_images: Vec::new(),
      maybe_seed: None,
      use_unlim: false,
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

  fn to_body(&self) -> Seedream5p0LiteRequestBody {
    Seedream5p0LiteRequestBody {
      params: Seedream5p0LiteParams {
        prompt: self.prompt.clone(),
        aspect_ratio: self.aspect_ratio,
        quality: self.resolution.to_image_quality(),
        batch_size: self.batch_size,
        use_unlim: self.use_unlim,
        medias: self.reference_images.iter().cloned().map(MediaReference::image).collect(),
        model: MODEL,
        width: PLACEHOLDER_WIDTH,
        height: PLACEHOLDER_HEIGHT,
        seed: self.maybe_seed.unwrap_or_else(ImageSeed::random),
      },
      use_unlim: self.use_unlim,
    }
  }
}

/// Enqueue the job. The response's job ids are what to poll (see
/// `endpoints::jobs`).
pub async fn seedream_5p0_lite(args: Seedream5p0LiteArgs<'_>) -> Result<EnqueueJobsResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body();
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct Seedream5p0LiteRequestBody {
  params: Seedream5p0LiteParams,
  use_unlim: bool,
}

#[derive(Serialize)]
struct Seedream5p0LiteParams {
  prompt: String,
  aspect_ratio: SeedreamAspectRatio,
  quality: ImageQuality,
  batch_size: ImageBatchSize,
  use_unlim: bool,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  medias: Vec<MediaReference>,
  model: &'static str,
  width: u32,
  height: u32,
  seed: ImageSeed,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::media_input::MediaInput;
  use crate::types::job_set_type::JobSetType;
  use serde_json::Value;

  /// Captured enqueue response (2K / basic), ids / user scrubbed.
  const ENQUEUE_RESPONSE: &str = r#"{"id":"00000000-0000-0000-0000-00000000aaaa","job_sets":[{"id":"00000000-0000-0000-0000-000000003940","type":"seedream_v5_lite","project_id":"00000000-0000-0000-0000-00000000aaaa","created_at":1788154819.250387,"parent_id":null,"cluster_hash":"d8cd56aedaaaeedac3a5ed262f5eed0e","cost":100,"params":{"prompt":"a corgi on a bike","medias":[],"batch_size":1,"aspect_ratio":"3:4","width":1728,"height":2304,"quality":"basic","seed":12745,"reference_elements":[]},"jobs":[{"id":"00000000-0000-0000-0000-0000f8f09c15","status":"queued","ip_check_finished":null,"ip_detected":null,"result":null,"results":null,"board_ids":[],"published_at":null,"meta":{},"created_at":1788154819.252099,"user_id":"user_TESTUSER0000000000000000000","trace_id":"00000000-0000-0000-0000-0000f8f09c15","cluster_hash":"d8cd56aedaaaeedac3a5ed262f5eed0e","representation":null,"folder_ids":[],"is_favourite":false}],"client_meta":null,"chain_id":null}],"has_more":false,"wallet":{"workspace_id":"00000000-0000-0000-0000-00000000aaaa","credits_balance":0,"subscription_balance":118850,"wallet_created_at":"2026-08-31T03:26:11.027426Z","next_credit_allocation_date":null,"total_credits":120000,"on_demand_credits":0,"expire_days":90},"workspace_details":{"id":"00000000-0000-0000-0000-00000000aaaa","name":null,"type":"private","user_role":"owner","clerk_organization_id":null,"avatar_url":null,"bio":null,"grace_period_type":null,"sso_status":"idle","is_enterprise_sub_workspace":false,"sub_workspace_block":null},"free_gens_v2":{"items":[]},"generation_seconds":{"items":[]},"folder_credits":null}"#;

  #[test]
  fn resolution_menu_maps_to_quality() {
    let mapping: Vec<(&str, String)> = Seedream5p0LiteResolution::all().iter().map(|r| (r.label(), r.to_image_quality().to_string())).collect();
    assert_eq!(mapping.iter().map(|(l, q)| (*l, q.as_str())).collect::<Vec<_>>(), [("2K", "basic"), ("3K", "high"), ("4K", "ultra")]);
  }

  #[test]
  fn wire_bodies_match_captured_requests() {
    // 2K / basic, 3K / high, 4K / ultra — each captured from the web app.
    let cases = [
      (Seedream5p0LiteResolution::TwoK, 12745u32, r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"3:4","quality":"basic","batch_size":1,"use_unlim":false,"model":"seedream_v5_lite","width":1024,"height":1024,"seed":12745},"use_unlim":false}"#),
      (Seedream5p0LiteResolution::ThreeK, 375239, r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"3:4","quality":"high","batch_size":1,"use_unlim":false,"model":"seedream_v5_lite","width":1024,"height":1024,"seed":375239},"use_unlim":false}"#),
      (Seedream5p0LiteResolution::FourK, 98746, r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"3:4","quality":"ultra","batch_size":1,"use_unlim":false,"model":"seedream_v5_lite","width":1024,"height":1024,"seed":98746},"use_unlim":false}"#),
    ];
    for (resolution, seed, expected) in cases {
      let mut request = Seedream5p0LiteRequest::text_to_image("a corgi on a bike", SeedreamAspectRatio::Portrait3x4, resolution);
      request.maybe_seed = Some(ImageSeed::new(seed));
      let actual: Value = serde_json::to_value(request.to_body()).unwrap();
      let expected: Value = serde_json::from_str(expected).unwrap();
      assert_eq!(actual, expected, "{}", resolution.label());
    }
  }

  #[test]
  fn wire_body_with_reference_image_matches_captured_request() {
    // Captured from the web app 2026-08-31 (ids scrubbed). The web app sent
    // the reference's 640x640 as width/height; this client keeps the
    // placeholder (the server derives the real size from `quality`).
    let mut request = Seedream5p0LiteRequest::text_to_image("a corgi on a bike", SeedreamAspectRatio::Portrait3x4, Seedream5p0LiteResolution::FourK)
        .with_reference_images(vec![MediaInput::uploaded("00000000-0000-4000-8000-0000000000aa", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png")]);
    request.maybe_seed = Some(ImageSeed::new(366541));
    let actual: Value = serde_json::to_value(request.to_body()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"3:4","quality":"ultra","batch_size":1,"use_unlim":false,"medias":[{"role":"image","data":{"id":"00000000-0000-4000-8000-0000000000aa","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png"}}],"model":"seedream_v5_lite","width":1024,"height":1024,"seed":366541},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn empty_prompt_is_rejected() {
    let request = Seedream5p0LiteRequest::text_to_image("", SeedreamAspectRatio::Square1x1, Seedream5p0LiteResolution::TwoK);
    assert!(matches!(request.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[tokio::test]
  async fn invalid_request_fails_before_any_http() {
    let auth = HiggsfieldAuth::new("token");
    let host = HiggsfieldHost::Custom("http://127.0.0.1:9".to_string());
    let request = Seedream5p0LiteRequest::text_to_image("", SeedreamAspectRatio::Square1x1, Seedream5p0LiteResolution::TwoK);
    let err = seedream_5p0_lite(Seedream5p0LiteArgs { request, auth: &auth, host: &host }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn enqueue_response_parses() {
    let response: EnqueueJobsResponse = serde_json::from_str(ENQUEUE_RESPONSE).unwrap();
    let job_set = response.first_job_set().unwrap();
    assert_eq!(job_set.job_set_type, JobSetType::SeedreamV5Lite);
    assert_eq!(job_set.cost, Some(100.0));
    assert_eq!(job_set.params.quality, Some(ImageQuality::Basic));
    // The server ignores the 1024x1024 placeholder and derives the size.
    assert_eq!((job_set.params.width, job_set.params.height), (Some(1728), Some(2304)));
  }

  // ── Live (ignored: needs a real session and spends credits) ──

  #[tokio::test]
  #[ignore]
  async fn live_enqueue_seedream_5p0_lite_from_app_credential_and_poll() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::poll_job_to_completion::poll_job_to_completion;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    let request = Seedream5p0LiteRequest::text_to_image("a corgi on a bike", SeedreamAspectRatio::Portrait3x4, Seedream5p0LiteResolution::TwoK);
    println!("\n===== request =====\n{:#?}", request);

    let response = seedream_5p0_lite(Seedream5p0LiteArgs { request, auth: &auth, host: &HiggsfieldHost::Higgsfield })
        .await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("\n===== POST {PATH} =====\n{:#?}", response);
    let job_ids = response.job_ids();
    assert_eq!(response.first_job_set().unwrap().job_set_type, JobSetType::SeedreamV5Lite);

    let job = poll_job_to_completion(&session, &job_ids[0]).await?;
    assert!(job.result_url().is_some());
    Ok(())
  }
}
