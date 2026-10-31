//! POST `/fnf/jobs/seedream-v4-5` — enqueue a Seedream 4.5 image job (job
//! set type `seedream_v4_5`). NB: not under `/v2/`, unlike the 5.0 models.
//!
//! Option sets below were read off the web app's image generator on
//! 2026-08-31: 8 aspect ratios, a 2K / 4K menu sent as `quality` basic /
//! high, 1–4 images. The app always sends `width`/`height` 1024×1024 and an
//! `input_images` list (reference images); the server derives the real size.

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
use crate::types::seedream_aspect_ratio::SeedreamAspectRatio;
use serde::Serialize;

const PATH: &str = "/fnf/jobs/seedream-v4-5";

/// The `model` field the web app sends on this endpoint.
const MODEL: &str = "seedream_v4_5";

/// The placeholder size the web app always sends; the server ignores it.
const PLACEHOLDER_WIDTH: u32 = 1024;
const PLACEHOLDER_HEIGHT: u32 = 1024;

/// The web app's resolution menu for Seedream 4.5. It goes out as the
/// `quality` param.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Seedream4p5Resolution {
  /// 2K → `basic`
  #[default]
  TwoK,
  /// 4K → `high`
  FourK,
}

impl Seedream4p5Resolution {
  pub fn all() -> [Self; 2] {
    [Self::TwoK, Self::FourK]
  }

  /// The menu label.
  pub fn label(self) -> &'static str {
    match self {
      Self::TwoK => "2K",
      Self::FourK => "4K",
    }
  }

  /// The `quality` the web app sends for this tier.
  pub fn to_image_quality(self) -> ImageQuality {
    match self {
      Self::TwoK => ImageQuality::Basic,
      Self::FourK => ImageQuality::High,
    }
  }
}

/// Serializes as the menu label ("2K" / "4K") so a logged request reads
/// the way the user chose it; the wire `quality` is derived at send.
impl Serialize for Seedream4p5Resolution {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.label())
  }
}

pub struct Seedream4p5Args<'a> {
  pub request: Seedream4p5Request,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// The material part of a Seedream 4.5 request. Serializable so it can be
/// logged or persisted separately from the session.
#[derive(Clone, Debug, Serialize)]
pub struct Seedream4p5Request {
  pub prompt: String,

  pub aspect_ratio: SeedreamAspectRatio,

  pub resolution: Seedream4p5Resolution,

  /// How many images to generate (1–4). Each costs credits.
  pub batch_size: ImageBatchSize,

  /// Reference images (image-to-image), uploaded first via
  /// `endpoints::media` / `HiggsfieldSession::upload_reference_media`.
  /// Empty for text-to-image. Sent as `input_images` (this endpoint
  /// predates the role-tagged `medias` list).
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

impl Seedream4p5Request {
  /// A text-to-image request with the web app's defaults (1 image, random
  /// seed, credits).
  pub fn text_to_image(prompt: impl Into<String>, aspect_ratio: SeedreamAspectRatio, resolution: Seedream4p5Resolution) -> Self {
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

  fn to_body(&self) -> Seedream4p5RequestBody {
    Seedream4p5RequestBody {
      params: Seedream4p5Params {
        prompt: self.prompt.clone(),
        aspect_ratio: self.aspect_ratio,
        quality: self.resolution.to_image_quality(),
        batch_size: self.batch_size,
        use_unlim: self.use_unlim,
        seed: self.maybe_seed.unwrap_or_else(ImageSeed::random),
        model: MODEL,
        width: PLACEHOLDER_WIDTH,
        height: PLACEHOLDER_HEIGHT,
        input_images: self.reference_images.clone(),
      },
      use_unlim: self.use_unlim,
    }
  }
}

/// Enqueue the job. The response's job ids are what to poll (see
/// `endpoints::jobs`).
pub async fn seedream_4p5(args: Seedream4p5Args<'_>) -> Result<EnqueueJobsResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body();
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct Seedream4p5RequestBody {
  params: Seedream4p5Params,
  use_unlim: bool,
}

#[derive(Serialize)]
struct Seedream4p5Params {
  prompt: String,
  aspect_ratio: SeedreamAspectRatio,
  quality: ImageQuality,
  batch_size: ImageBatchSize,
  use_unlim: bool,
  seed: ImageSeed,
  model: &'static str,
  width: u32,
  height: u32,
  input_images: Vec<MediaInput>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::media_input::MediaInput;
  use crate::types::job_set_type::JobSetType;
  use serde_json::Value;

  /// Captured enqueue response (2K / basic), ids / user scrubbed. NB: no
  /// `cluster_hash` on this pipeline.
  const ENQUEUE_RESPONSE: &str = r#"{"id":"00000000-0000-0000-0000-00000000aaaa","job_sets":[{"id":"00000000-0000-0000-0000-0000000020bd","type":"seedream_v4_5","project_id":"00000000-0000-0000-0000-00000000aaaa","created_at":1788154912.273379,"parent_id":null,"cost":100,"params":{"prompt":"a corgi on a bike","batch_size":1,"quality":"basic","aspect_ratio":"3:4","seed":447696,"width":1728,"height":2304,"input_images":[],"use_unlim":false,"reference_elements":[]},"jobs":[{"id":"00000000-0000-0000-0000-000087a44a5a","status":"queued","ip_check_finished":null,"ip_detected":null,"result":null,"results":null,"board_ids":[],"published_at":null,"meta":{},"created_at":1788154912.278016,"user_id":"user_TESTUSER0000000000000000000","trace_id":"00000000-0000-0000-0000-000087a44a5a","folder_ids":[],"is_favourite":false}],"client_meta":null,"chain_id":null}],"has_more":false,"wallet":{"workspace_id":"00000000-0000-0000-0000-00000000aaaa","credits_balance":0,"subscription_balance":118750,"wallet_created_at":"2026-08-31T03:26:11.027426Z","next_credit_allocation_date":null,"total_credits":120000,"on_demand_credits":0,"expire_days":90},"workspace_details":{"id":"00000000-0000-0000-0000-00000000aaaa","name":null,"type":"private","user_role":"owner","clerk_organization_id":null,"avatar_url":null,"bio":null,"grace_period_type":null,"sso_status":"idle","is_enterprise_sub_workspace":false,"sub_workspace_block":null},"free_gens_v2":{"items":[]},"generation_seconds":{"items":[]},"folder_credits":null}"#;

  #[test]
  fn resolution_menu_maps_to_quality() {
    let mapping: Vec<(&str, String)> = Seedream4p5Resolution::all().iter().map(|r| (r.label(), r.to_image_quality().to_string())).collect();
    assert_eq!(mapping.iter().map(|(l, q)| (*l, q.as_str())).collect::<Vec<_>>(), [("2K", "basic"), ("4K", "high")]);
  }

  #[test]
  fn wire_bodies_match_captured_requests() {
    let cases = [
      (Seedream4p5Resolution::TwoK, 447696u32, r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"3:4","quality":"basic","batch_size":1,"use_unlim":false,"seed":447696,"model":"seedream_v4_5","width":1024,"height":1024,"input_images":[]},"use_unlim":false}"#),
      (Seedream4p5Resolution::FourK, 69127, r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"3:4","quality":"high","batch_size":1,"use_unlim":false,"seed":69127,"model":"seedream_v4_5","width":1024,"height":1024,"input_images":[]},"use_unlim":false}"#),
    ];
    for (resolution, seed, expected) in cases {
      let mut request = Seedream4p5Request::text_to_image("a corgi on a bike", SeedreamAspectRatio::Portrait3x4, resolution);
      request.maybe_seed = Some(ImageSeed::new(seed));
      let actual: Value = serde_json::to_value(request.to_body()).unwrap();
      let expected: Value = serde_json::from_str(expected).unwrap();
      assert_eq!(actual, expected, "{}", resolution.label());
    }
  }

  #[test]
  fn wire_body_with_reference_image_matches_captured_request() {
    // Captured from the web app 2026-08-31 (ids scrubbed): bare descriptors
    // in `input_images`. The web app sent the reference's 640x640 as
    // width/height; this client keeps the placeholder.
    let mut request = Seedream4p5Request::text_to_image("a corgi on a bike", SeedreamAspectRatio::Portrait3x4, Seedream4p5Resolution::FourK)
        .with_reference_images(vec![MediaInput::uploaded("00000000-0000-4000-8000-0000000000aa", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png")]);
    request.maybe_seed = Some(ImageSeed::new(275906));
    let actual: Value = serde_json::to_value(request.to_body()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"3:4","quality":"high","batch_size":1,"use_unlim":false,"seed":275906,"model":"seedream_v4_5","width":1024,"height":1024,"input_images":[{"id":"00000000-0000-4000-8000-0000000000aa","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png"}]},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn empty_prompt_is_rejected() {
    let request = Seedream4p5Request::text_to_image("", SeedreamAspectRatio::Square1x1, Seedream4p5Resolution::TwoK);
    assert!(matches!(request.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[tokio::test]
  async fn invalid_request_fails_before_any_http() {
    let auth = HiggsfieldAuth::new("token");
    let host = HiggsfieldHost::Custom("http://127.0.0.1:9".to_string());
    let request = Seedream4p5Request::text_to_image("", SeedreamAspectRatio::Square1x1, Seedream4p5Resolution::TwoK);
    let err = seedream_4p5(Seedream4p5Args { request, auth: &auth, host: &host }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn enqueue_response_parses() {
    let response: EnqueueJobsResponse = serde_json::from_str(ENQUEUE_RESPONSE).unwrap();
    let job_set = response.first_job_set().unwrap();
    assert_eq!(job_set.job_set_type, JobSetType::SeedreamV4p5);
    assert_eq!(job_set.params.quality, Some(ImageQuality::Basic));
    assert_eq!(job_set.params.seed, Some(ImageSeed::new(447696)));
    assert!(job_set.jobs[0].cluster_hash.is_none());
  }

  // ── Live (ignored: needs a real session and spends credits) ──

  #[tokio::test]
  #[ignore]
  async fn live_enqueue_seedream_4p5_from_app_credential_and_poll() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::poll_job_to_completion::poll_job_to_completion;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    let request = Seedream4p5Request::text_to_image("a corgi on a bike", SeedreamAspectRatio::Portrait3x4, Seedream4p5Resolution::TwoK);
    println!("\n===== request =====\n{:#?}", request);

    let response = seedream_4p5(Seedream4p5Args { request, auth: &auth, host: &HiggsfieldHost::Higgsfield })
        .await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("\n===== POST {PATH} =====\n{:#?}", response);
    let job_ids = response.job_ids();
    assert_eq!(response.first_job_set().unwrap().job_set_type, JobSetType::SeedreamV4p5);

    let job = poll_job_to_completion(&session, &job_ids[0]).await?;
    assert!(job.result_url().is_some());
    Ok(())
  }
}
