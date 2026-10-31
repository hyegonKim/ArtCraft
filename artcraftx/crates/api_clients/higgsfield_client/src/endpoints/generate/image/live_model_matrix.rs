//! Live, credit-spending tests that exercise every image binding through
//! [`HiggsfieldSession`](crate::session::higgsfield_session::HiggsfieldSession)
//! off the desktop app's saved login, then follow each job through the
//! status endpoints. One test per model so failures isolate; run them
//! serially so the output stays readable:
//!
//! ```text
//! cargo test -p higgsfield_client live_matrix -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Mix: single images at the high end of each quality menu, and maximum
//! batches (4) at the low end.

use crate::endpoints::generate::image::gpt_image_2::{GptImage2AspectRatio, GptImage2Quality, GptImage2Request, GptImage2Resolution};
use crate::endpoints::generate::image::nano_banana_2::{NanoBanana2Request, NanoBanana2Resolution};
use crate::endpoints::generate::image::nano_banana_2_lite::{NanoBanana2LiteQuality, NanoBanana2LiteRequest};
use crate::endpoints::generate::image::nano_banana_pro::{NanoBananaProRequest, NanoBananaProResolution};
use crate::endpoints::generate::image::seedream_4p5::{Seedream4p5Request, Seedream4p5Resolution};
use crate::endpoints::generate::image::seedream_5p0_lite::{Seedream5p0LiteRequest, Seedream5p0LiteResolution};
use crate::endpoints::generate::image::seedream_5p0_pro::{Seedream5p0ProRequest, Seedream5p0ProResolution};
use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
use crate::test_utils::poll_job_to_completion::poll_jobs_to_completion;
use crate::test_utils::setup_test_logging::setup_test_logging;
use crate::types::enqueue_jobs_response::EnqueueJobsResponse;
use crate::types::image_batch_size::ImageBatchSize;
use crate::types::job_set_type::JobSetType;
use crate::types::nano_banana_aspect_ratio::NanoBananaAspectRatio;
use crate::types::seedream_aspect_ratio::SeedreamAspectRatio;
use crate::session::higgsfield_session::HiggsfieldSession;

const PROMPT_SKATEBOARD: &str = "a shiba inu doing a kickflip on a skateboard at a skate park, golden hour";
const PROMPT_SURF: &str = "a shiba inu surfing a big wave, action photo";
const PROMPT_SOCCER: &str = "a shiba inu scoring a goal in a soccer match, stadium lights";
const PROMPT_BASKETBALL: &str = "a shiba inu dunking a basketball, slow motion";
const PROMPT_SKI: &str = "a shiba inu skiing down a mountain, spraying powder";
const PROMPT_TENNIS: &str = "a shiba inu serving a tennis ball at Wimbledon";
const PROMPT_CYCLING: &str = "a shiba inu winning a bicycle race, crossing the finish line";

fn session() -> anyhow::Result<HiggsfieldSession> {
  setup_test_logging();
  load_higgsfield_session_from_app_credential()
}

/// Print the enqueue, check the job set, and follow every job to completion.
async fn follow(session: &HiggsfieldSession, label: &str, expected_type: JobSetType, expected_jobs: usize, response: EnqueueJobsResponse) -> anyhow::Result<()> {
  let job_set = response.first_job_set().ok_or_else(|| anyhow::anyhow!("no job set"))?;
  println!(
    "\n##### {label}: enqueued job set {} type={} cost={:?} jobs={} server size={:?}x{:?}",
    job_set.id, job_set.job_set_type, job_set.cost, job_set.jobs.len(), job_set.params.width, job_set.params.height,
  );
  println!("wallet after: {:?}", response.wallet.as_ref().map(|w| (w.credits_balance, w.subscription_balance)));
  assert_eq!(job_set.job_set_type, expected_type);
  assert_eq!(job_set.jobs.len(), expected_jobs, "batch size should fan out into one job per image");

  let job_ids = response.job_ids();
  let jobs = poll_jobs_to_completion(session, &job_ids).await?;
  assert_eq!(jobs.len(), expected_jobs);
  for job in &jobs {
    assert_eq!(job.job_set_type, expected_type);
    assert!(job.result_url().is_some(), "job {} has no result url", job.id);
  }
  println!("##### {label}: {} image(s) done", jobs.len());
  Ok(())
}

// ── High quality, single image ──

#[tokio::test]
#[ignore]
async fn live_matrix_seedream_5p0_pro_2k() -> anyhow::Result<()> {
  let session = session()?;
  let request = Seedream5p0ProRequest::text_to_image(PROMPT_SKATEBOARD, SeedreamAspectRatio::Landscape16x9, Seedream5p0ProResolution::TwoK);
  let response = session.seedream_5p0_pro(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Seedream 5.0 Pro @ 2K", JobSetType::SeedreamV5Pro, 1, response).await
}

#[tokio::test]
#[ignore]
async fn live_matrix_seedream_5p0_lite_4k_ultra() -> anyhow::Result<()> {
  let session = session()?;
  let request = Seedream5p0LiteRequest::text_to_image(PROMPT_SURF, SeedreamAspectRatio::Landscape3x2, Seedream5p0LiteResolution::FourK);
  let response = session.seedream_5p0_lite(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Seedream 5.0 lite @ 4K (ultra)", JobSetType::SeedreamV5Lite, 1, response).await
}

#[tokio::test]
#[ignore]
async fn live_matrix_nano_banana_pro_2k() -> anyhow::Result<()> {
  let session = session()?;
  let request = NanoBananaProRequest::text_to_image(PROMPT_SOCCER, NanoBananaAspectRatio::Landscape16x9, NanoBananaProResolution::TwoK);
  let response = session.nano_banana_pro(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Nano Banana Pro @ 2K", JobSetType::NanoBanana2, 1, response).await
}

#[tokio::test]
#[ignore]
async fn live_matrix_gpt_image_2_high_1k() -> anyhow::Result<()> {
  let session = session()?;
  let request = GptImage2Request::text_to_image(PROMPT_BASKETBALL, GptImage2AspectRatio::Portrait3x4, GptImage2Quality::High, GptImage2Resolution::OneK);
  let response = session.gpt_image_2(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "GPT Image 2 @ 1K high", JobSetType::GptImage2, 1, response).await
}

// ── Maximum batch (4), low quality ──

#[tokio::test]
#[ignore]
async fn live_matrix_seedream_4p5_2k_batch_of_4() -> anyhow::Result<()> {
  let session = session()?;
  let mut request = Seedream4p5Request::text_to_image(PROMPT_SKI, SeedreamAspectRatio::Square1x1, Seedream4p5Resolution::TwoK);
  request.batch_size = ImageBatchSize::Four;
  let response = session.seedream_4p5(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Seedream 4.5 @ 2K (basic) x4", JobSetType::SeedreamV4p5, 4, response).await
}

#[tokio::test]
#[ignore]
async fn live_matrix_nano_banana_2_1k_batch_of_4() -> anyhow::Result<()> {
  let session = session()?;
  let mut request = NanoBanana2Request::text_to_image(PROMPT_TENNIS, NanoBananaAspectRatio::Portrait3x4, NanoBanana2Resolution::OneK);
  request.batch_size = ImageBatchSize::Four;
  let response = session.nano_banana_2(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Nano Banana 2 @ 1K x4", JobSetType::NanoBananaFlash, 4, response).await
}

#[tokio::test]
#[ignore]
async fn live_matrix_nano_banana_2_lite_minimal_batch_of_4() -> anyhow::Result<()> {
  let session = session()?;
  let mut request = NanoBanana2LiteRequest::text_to_image(PROMPT_CYCLING, NanoBananaAspectRatio::Landscape4x3, NanoBanana2LiteQuality::Minimal);
  request.batch_size = ImageBatchSize::Four;
  let response = session.nano_banana_2_lite(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Nano Banana 2 Lite (MINIMAL) x4", JobSetType::NanoBanana2Lite, 4, response).await
}
