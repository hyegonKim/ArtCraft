//! POST `/fnf/jobs/v2/seedream_v5_pro` — enqueue a Seedream 5.0 Pro image
//! job (job set type `seedream_v5_pro`).
//!
//! Option sets below were read off the web app's image generator on
//! 2026-08-31: 8 aspect ratios, 1K / 1.5K / 2K, 1–4 images. Unlike the Nano
//! Banana endpoints, no width/height is sent — the server derives the size.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::enqueue_jobs_response::EnqueueJobsResponse;
use crate::types::image_batch_size::ImageBatchSize;
use crate::types::image_seed::ImageSeed;
use crate::types::media_input::MediaInput;
use crate::types::media_reference::MediaReference;
use crate::types::seedream_aspect_ratio::SeedreamAspectRatio;
use serde::Serialize;

const PATH: &str = "/fnf/jobs/v2/seedream_v5_pro";

/// The resolution tiers the web app offers for Seedream 5.0 Pro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Seedream5p0ProResolution {
  #[default]
  OneK,
  OnePointFiveK,
  TwoK,
}

impl Seedream5p0ProResolution {
  pub fn all() -> [Self; 3] {
    [Self::OneK, Self::OnePointFiveK, Self::TwoK]
  }

  pub fn as_str(self) -> &'static str {
    match self {
      Self::OneK => "1k",
      Self::OnePointFiveK => "1.5k",
      Self::TwoK => "2k",
    }
  }
}

impl Serialize for Seedream5p0ProResolution {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.as_str())
  }
}

pub struct Seedream5p0ProArgs<'a> {
  pub request: Seedream5p0ProRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// The material part of a Seedream 5.0 Pro request. Serializable so it can
/// be logged or persisted separately from the session.
#[derive(Clone, Debug, Serialize)]
pub struct Seedream5p0ProRequest {
  pub prompt: String,

  pub aspect_ratio: SeedreamAspectRatio,

  pub resolution: Seedream5p0ProResolution,

  /// How many images to generate (1–4). Each costs credits.
  pub batch_size: ImageBatchSize,

  /// Reference images (image-to-image), uploaded first via
  /// `endpoints::media` / `HiggsfieldSession::upload_reference_media`.
  /// Empty for text-to-image (then `medias` is omitted, as the web app
  /// does). Sent as `medias` with role `image`.
  pub reference_images: Vec<MediaInput>,

  /// Pin the generation seed; `None` sends a fresh random one, as the web
  /// app does.
  pub maybe_seed: Option<ImageSeed>,

  /// Spend from the plan's "unlimited" pool instead of credits, if the plan
  /// has one.
  pub use_unlim: bool,
}

impl Seedream5p0ProRequest {
  /// A text-to-image request with the web app's defaults (1 image, random
  /// seed, credits).
  pub fn text_to_image(prompt: impl Into<String>, aspect_ratio: SeedreamAspectRatio, resolution: Seedream5p0ProResolution) -> Self {
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

  fn to_body(&self) -> Seedream5p0ProRequestBody {
    Seedream5p0ProRequestBody {
      params: Seedream5p0ProParams {
        prompt: self.prompt.clone(),
        aspect_ratio: self.aspect_ratio,
        resolution: self.resolution,
        batch_size: self.batch_size,
        use_unlim: self.use_unlim,
        medias: self.reference_images.iter().cloned().map(MediaReference::image).collect(),
        seed: self.maybe_seed.unwrap_or_else(ImageSeed::random),
      },
      use_unlim: self.use_unlim,
    }
  }
}

/// Enqueue the job. The response's job ids are what to poll (see
/// `endpoints::jobs`).
pub async fn seedream_5p0_pro(args: Seedream5p0ProArgs<'_>) -> Result<EnqueueJobsResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body();
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct Seedream5p0ProRequestBody {
  params: Seedream5p0ProParams,
  use_unlim: bool,
}

#[derive(Serialize)]
struct Seedream5p0ProParams {
  prompt: String,
  aspect_ratio: SeedreamAspectRatio,
  resolution: Seedream5p0ProResolution,
  batch_size: ImageBatchSize,
  use_unlim: bool,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  medias: Vec<MediaReference>,
  seed: ImageSeed,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::media_input::MediaInput;
  use crate::types::job_set_type::JobSetType;
  use serde_json::Value;

  /// Captured enqueue response, ids / user scrubbed.
  const ENQUEUE_RESPONSE: &str = r#"{"id":"00000000-0000-0000-0000-00000000aaaa","job_sets":[{"id":"00000000-0000-0000-0000-000000005f0e","type":"seedream_v5_pro","project_id":"00000000-0000-0000-0000-00000000aaaa","created_at":1788154712.937005,"parent_id":null,"cluster_hash":"d8cd56aedaaaeedac3a5ed262f5eed0e","cost":150,"params":{"prompt":"a corgi on a bike","medias":[],"batch_size":1,"aspect_ratio":"3:4","width":880,"height":1168,"resolution":"1k","seed":158368,"is_inpaint":false,"remove_bg":false,"reference_elements":[]},"jobs":[{"id":"00000000-0000-0000-0000-0000b78db500","status":"queued","ip_check_finished":null,"ip_detected":null,"result":null,"results":null,"board_ids":[],"published_at":null,"meta":{},"created_at":1788154712.944875,"user_id":"user_TESTUSER0000000000000000000","trace_id":"00000000-0000-0000-0000-0000b78db500","cluster_hash":"d8cd56aedaaaeedac3a5ed262f5eed0e","representation":null,"folder_ids":[],"is_favourite":false}],"client_meta":null,"chain_id":null}],"has_more":false,"wallet":{"workspace_id":"00000000-0000-0000-0000-00000000aaaa","credits_balance":0,"subscription_balance":118950,"wallet_created_at":"2026-08-31T03:26:11.027426Z","next_credit_allocation_date":null,"total_credits":120000,"on_demand_credits":0,"expire_days":90},"workspace_details":{"id":"00000000-0000-0000-0000-00000000aaaa","name":null,"type":"private","user_role":"owner","clerk_organization_id":null,"avatar_url":null,"bio":null,"grace_period_type":null,"sso_status":"idle","is_enterprise_sub_workspace":false,"sub_workspace_block":null},"free_gens_v2":{"items":[]},"generation_seconds":{"items":[]},"folder_credits":null}"#;

  #[test]
  fn resolutions_match_the_web_app_menu() {
    let wire: Vec<&str> = Seedream5p0ProResolution::all().iter().map(|r| r.as_str()).collect();
    assert_eq!(wire, ["1k", "1.5k", "2k"]);
  }

  #[test]
  fn wire_body_matches_captured_request() {
    // Captured from the web app: 3:4 at 1k, seed pinned to the captured one.
    let mut request = Seedream5p0ProRequest::text_to_image("a corgi on a bike", SeedreamAspectRatio::Portrait3x4, Seedream5p0ProResolution::OneK);
    request.maybe_seed = Some(ImageSeed::new(158368));
    let actual: Value = serde_json::to_value(request.to_body()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"3:4","resolution":"1k","batch_size":1,"use_unlim":false,"seed":158368},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn wire_body_1p5k_matches_captured_request() {
    let mut request = Seedream5p0ProRequest::text_to_image("a corgi on a bike", SeedreamAspectRatio::Portrait3x4, Seedream5p0ProResolution::OnePointFiveK);
    request.maybe_seed = Some(ImageSeed::new(132934));
    let actual: Value = serde_json::to_value(request.to_body()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"3:4","resolution":"1.5k","batch_size":1,"use_unlim":false,"seed":132934},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn unpinned_seed_is_random_and_in_range() {
    let request = Seedream5p0ProRequest::text_to_image("p", SeedreamAspectRatio::Square1x1, Seedream5p0ProResolution::OneK);
    let body: Value = serde_json::to_value(request.to_body()).unwrap();
    let seed = body["params"]["seed"].as_u64().unwrap();
    assert!(seed < 1_000_000);
  }

  #[test]
  fn wire_body_with_reference_image_matches_captured_request() {
    // Captured from the web app 2026-08-31 (ids scrubbed).
    let mut request = Seedream5p0ProRequest::text_to_image("a corgi on a bike", SeedreamAspectRatio::Portrait3x4, Seedream5p0ProResolution::OnePointFiveK)
        .with_reference_images(vec![MediaInput::uploaded("00000000-0000-4000-8000-0000000000aa", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png")]);
    request.maybe_seed = Some(ImageSeed::new(192164));
    let actual: Value = serde_json::to_value(request.to_body()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a corgi on a bike","aspect_ratio":"3:4","resolution":"1.5k","batch_size":1,"use_unlim":false,"medias":[{"role":"image","data":{"id":"00000000-0000-4000-8000-0000000000aa","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png"}}],"seed":192164},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn empty_prompt_is_rejected() {
    let request = Seedream5p0ProRequest::text_to_image(" ", SeedreamAspectRatio::Square1x1, Seedream5p0ProResolution::OneK);
    assert!(matches!(request.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[tokio::test]
  async fn invalid_request_fails_before_any_http() {
    let auth = HiggsfieldAuth::new("token");
    let host = HiggsfieldHost::Custom("http://127.0.0.1:9".to_string());
    let request = Seedream5p0ProRequest::text_to_image("", SeedreamAspectRatio::Square1x1, Seedream5p0ProResolution::OneK);
    let err = seedream_5p0_pro(Seedream5p0ProArgs { request, auth: &auth, host: &host }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn enqueue_response_parses() {
    let response: EnqueueJobsResponse = serde_json::from_str(ENQUEUE_RESPONSE).unwrap();
    let job_set = response.first_job_set().unwrap();
    assert_eq!(job_set.job_set_type, JobSetType::SeedreamV5Pro);
    assert_eq!(job_set.cost, Some(150.0));
    assert_eq!(job_set.params.seed, Some(ImageSeed::new(158368)));
    assert_eq!((job_set.params.width, job_set.params.height), (Some(880), Some(1168)));
    assert_eq!(response.job_ids().len(), 1);
  }

  // ── Live (ignored: needs a real session and spends credits) ──

  /// Enqueues off the desktop app's saved Higgsfield login, prints the
  /// enqueue response, then follows the job to completion. Cheapest
  /// settings (1 image, 1k).
  #[tokio::test]
  #[ignore]
  async fn live_enqueue_seedream_5p0_pro_from_app_credential_and_poll() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::poll_job_to_completion::poll_job_to_completion;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    let request = Seedream5p0ProRequest::text_to_image("a corgi on a bike", SeedreamAspectRatio::Portrait3x4, Seedream5p0ProResolution::OneK);
    println!("\n===== request =====\n{:#?}", request);

    let response = seedream_5p0_pro(Seedream5p0ProArgs { request, auth: &auth, host: &HiggsfieldHost::Higgsfield })
        .await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("\n===== POST {PATH} =====\n{:#?}", response);
    let job_ids = response.job_ids();
    assert_eq!(response.first_job_set().unwrap().job_set_type, JobSetType::SeedreamV5Pro);

    let job = poll_job_to_completion(&session, &job_ids[0]).await?;
    assert!(job.result_url().is_some());
    Ok(())
  }
}
