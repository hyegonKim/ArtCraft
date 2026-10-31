//! Every endpoint as a [`HiggsfieldSession`] method: the session supplies a
//! fresh bearer token and retries once on `401`.

use crate::endpoints::generate::image::gpt_image_2::{gpt_image_2, GptImage2Args, GptImage2Request};
use crate::endpoints::generate::image::nano_banana_2::{nano_banana_2, NanoBanana2Args, NanoBanana2Request};
use crate::endpoints::generate::image::nano_banana_2_lite::{nano_banana_2_lite, NanoBanana2LiteArgs, NanoBanana2LiteRequest};
use crate::endpoints::generate::image::nano_banana_pro::{nano_banana_pro, NanoBananaProArgs, NanoBananaProRequest};
use crate::endpoints::generate::image::seedream_4p5::{seedream_4p5, Seedream4p5Args, Seedream4p5Request};
use crate::endpoints::generate::image::seedream_5p0_lite::{seedream_5p0_lite, Seedream5p0LiteArgs, Seedream5p0LiteRequest};
use crate::endpoints::generate::image::seedream_5p0_pro::{seedream_5p0_pro, Seedream5p0ProArgs, Seedream5p0ProRequest};
use crate::endpoints::generate::video::grok_imagine_1p5::{grok_imagine_1p5, GrokImagine1p5Args, GrokImagine1p5Request};
use crate::endpoints::generate::video::kling_3p0::{kling_3p0, Kling3p0Args, Kling3p0Request};
use crate::endpoints::generate::video::minimax_h3::{minimax_h3, MinimaxH3Args, MinimaxH3Request};
use crate::endpoints::generate::video::seedance_2p0::{seedance_2p0, Seedance2p0Args, Seedance2p0Request};
use crate::endpoints::generate::video::seedance_2p0_mini::{seedance_2p0_mini, Seedance2p0MiniArgs, Seedance2p0MiniRequest};
use crate::endpoints::generate::video::seedance_2p5::{seedance_2p5, Seedance2p5Args, Seedance2p5Request};
use crate::endpoints::generate::video::seedance_2p5_edit::{seedance_2p5_edit, Seedance2p5EditArgs, Seedance2p5EditRequest};
use crate::endpoints::jobs::job_status::{job_status, JobStatusArgs, JobStatusRequest, JobStatusResponse};
use crate::endpoints::jobs::job_status_batch::{job_status_batch, JobStatusBatchArgs, JobStatusBatchRequest, JobStatusBatchResponse};
use crate::endpoints::user::user_data::{user_data, UserDataArgs, UserDataRequest, UserDataResponse};
use crate::endpoints::user::user_profile::{user_profile, UserProfileArgs, UserProfileRequest, UserProfileResponse};
use crate::error::higgsfield_error::HiggsfieldError;
use crate::session::higgsfield_session::HiggsfieldSession;
use crate::types::enqueue_jobs_response::EnqueueJobsResponse;
use crate::types::ids::JobId;

impl HiggsfieldSession {
  /// Enqueue a Nano Banana Pro image job.
  pub async fn nano_banana_pro(&self, request: NanoBananaProRequest) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = request.clone();
      async move { nano_banana_pro(NanoBananaProArgs { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Enqueue a Nano Banana 2 image job.
  pub async fn nano_banana_2(&self, request: NanoBanana2Request) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = request.clone();
      async move { nano_banana_2(NanoBanana2Args { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Enqueue a Nano Banana 2 Lite image job.
  pub async fn nano_banana_2_lite(&self, request: NanoBanana2LiteRequest) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = request.clone();
      async move { nano_banana_2_lite(NanoBanana2LiteArgs { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Enqueue a Seedream 5.0 Pro image job.
  pub async fn seedream_5p0_pro(&self, request: Seedream5p0ProRequest) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = request.clone();
      async move { seedream_5p0_pro(Seedream5p0ProArgs { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Enqueue a Seedream 5.0 lite image job.
  pub async fn seedream_5p0_lite(&self, request: Seedream5p0LiteRequest) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = request.clone();
      async move { seedream_5p0_lite(Seedream5p0LiteArgs { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Enqueue a Seedream 4.5 image job.
  pub async fn seedream_4p5(&self, request: Seedream4p5Request) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = request.clone();
      async move { seedream_4p5(Seedream4p5Args { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Enqueue a GPT Image job.
  pub async fn gpt_image_2(&self, request: GptImage2Request) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = request.clone();
      async move { gpt_image_2(GptImage2Args { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  // ── Video ──

  /// Enqueue a Seedance 2.5 video job.
  pub async fn seedance_2p5(&self, request: Seedance2p5Request) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = request.clone();
      async move { seedance_2p5(Seedance2p5Args { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Enqueue a Seedance 2.5 Edit (video-to-video) job.
  pub async fn seedance_2p5_edit(&self, request: Seedance2p5EditRequest) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = request.clone();
      async move { seedance_2p5_edit(Seedance2p5EditArgs { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Enqueue a Seedance 2.0 video job.
  pub async fn seedance_2p0(&self, request: Seedance2p0Request) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = request.clone();
      async move { seedance_2p0(Seedance2p0Args { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Enqueue a Seedance 2.0 Mini video job.
  pub async fn seedance_2p0_mini(&self, request: Seedance2p0MiniRequest) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = request.clone();
      async move { seedance_2p0_mini(Seedance2p0MiniArgs { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Enqueue a MiniMax H3 video job.
  pub async fn minimax_h3(&self, request: MinimaxH3Request) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = request.clone();
      async move { minimax_h3(MinimaxH3Args { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Enqueue a Kling 3.0 video job.
  pub async fn kling_3p0(&self, request: Kling3p0Request) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = request.clone();
      async move { kling_3p0(Kling3p0Args { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Enqueue a Grok Imagine 1.5 video job.
  pub async fn grok_imagine_1p5(&self, request: GrokImagine1p5Request) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = request.clone();
      async move { grok_imagine_1p5(GrokImagine1p5Args { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Full state of one job, including result URLs once complete.
  pub async fn job_status(&self, job_id: &JobId) -> Result<JobStatusResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = JobStatusRequest { job_id: job_id.clone() };
      async move { job_status(JobStatusArgs { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Lightweight status for many jobs.
  pub async fn job_status_batch(&self, job_ids: &[JobId]) -> Result<JobStatusBatchResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = JobStatusBatchRequest { ids: job_ids.to_vec() };
      async move { job_status_batch(JobStatusBatchArgs { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// The account: plan, credits, workspace. Also the cheapest session check.
  pub async fn user_data(&self) -> Result<UserDataResponse, HiggsfieldError> {
    self.with_auth(|auth| async move {
      user_data(UserDataArgs { request: UserDataRequest, auth: &auth, host: self.api_host() }).await
    }).await
  }

  /// The public profile.
  pub async fn user_profile(&self) -> Result<UserProfileResponse, HiggsfieldError> {
    self.with_auth(|auth| async move {
      user_profile(UserProfileArgs { request: UserProfileRequest, auth: &auth, host: self.api_host() }).await
    }).await
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::client::clerk_host::ClerkHost;
  use crate::client::higgsfield_host::HiggsfieldHost;
  use crate::error::higgsfield_client_error::HiggsfieldClientError;
  use crate::endpoints::generate::image::gpt_image_2::{GptImage2AspectRatio, GptImage2Quality, GptImage2Resolution};
  use crate::endpoints::generate::image::nano_banana_pro::{NanoBananaProAspectRatio, NanoBananaProResolution};

  /// A session that can't reach anything, but has a fresh seed token so the
  /// wrappers get past auth and into request validation.
  fn offline_session() -> HiggsfieldSession {
    use crate::credentials::clerk_session_token::tests::fake_clerk_token;
    let token = fake_clerk_token((chrono::Utc::now() + chrono::Duration::hours(1)).timestamp());
    HiggsfieldSession::from_cookie_header("__client=x")
        .with_hosts(HiggsfieldHost::Custom("http://127.0.0.1:9".into()), ClerkHost::Custom("http://127.0.0.1:9".into()))
        .with_initial_token(token)
  }

  #[tokio::test]
  async fn wrappers_apply_request_validation() {
    let session = offline_session();

    let request = NanoBananaProRequest::text_to_image("", NanoBananaProAspectRatio::Square1x1, NanoBananaProResolution::OneK);
    let err = session.nano_banana_pro(request).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));

    let request = GptImage2Request::text_to_image("", GptImage2AspectRatio::Square1x1, GptImage2Quality::Low, GptImage2Resolution::OneK);
    let err = session.gpt_image_2(request).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));

    use crate::endpoints::generate::image::nano_banana_2::NanoBanana2Resolution;
    use crate::endpoints::generate::image::nano_banana_2_lite::NanoBanana2LiteQuality;
    use crate::endpoints::generate::image::seedream_4p5::Seedream4p5Resolution;
    use crate::endpoints::generate::image::seedream_5p0_lite::Seedream5p0LiteResolution;
    use crate::endpoints::generate::image::seedream_5p0_pro::Seedream5p0ProResolution;
    use crate::types::seedream_aspect_ratio::SeedreamAspectRatio;
    let err = session.nano_banana_2(NanoBanana2Request::text_to_image("", NanoBananaProAspectRatio::Square1x1, NanoBanana2Resolution::OneK)).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
    let err = session.nano_banana_2_lite(NanoBanana2LiteRequest::text_to_image("", NanoBananaProAspectRatio::Square1x1, NanoBanana2LiteQuality::High)).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
    let err = session.seedream_5p0_pro(Seedream5p0ProRequest::text_to_image("", SeedreamAspectRatio::Square1x1, Seedream5p0ProResolution::OneK)).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
    let err = session.seedream_5p0_lite(Seedream5p0LiteRequest::text_to_image("", SeedreamAspectRatio::Square1x1, Seedream5p0LiteResolution::TwoK)).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
    let err = session.seedream_4p5(Seedream4p5Request::text_to_image("", SeedreamAspectRatio::Square1x1, Seedream4p5Resolution::TwoK)).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));

    use crate::endpoints::generate::video::grok_imagine_1p5::GrokImagine1p5Resolution;
    use crate::endpoints::generate::video::kling_3p0::Kling3p0Resolution;
    use crate::endpoints::generate::video::seedance_2p0::Seedance2p0Resolution;
    use crate::endpoints::generate::video::seedance_2p0_mini::Seedance2p0MiniResolution;
    use crate::endpoints::generate::video::seedance_2p5::Seedance2p5Resolution;
    use crate::types::video_aspect_ratio::{KlingAspectRatio, SeedanceVideoAspectRatio};
    use crate::types::video_duration::VideoDurationSeconds;
    let bad_duration = VideoDurationSeconds::new(0);
    let err = session.seedance_2p5(Seedance2p5Request::text_to_video("p", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p5Resolution::P480, bad_duration)).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
    let err = session.seedance_2p0(Seedance2p0Request::text_to_video("p", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p0Resolution::P480, bad_duration)).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
    let err = session.seedance_2p0_mini(Seedance2p0MiniRequest::text_to_video("p", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p0MiniResolution::P480, bad_duration)).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
    let err = session.minimax_h3(MinimaxH3Request::text_to_video("p", bad_duration)).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
    let err = session.kling_3p0(Kling3p0Request::text_to_video("p", KlingAspectRatio::Landscape16x9, Kling3p0Resolution::P720, bad_duration)).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
    let err = session.grok_imagine_1p5(GrokImagine1p5Request::text_to_video("p", GrokImagine1p5Resolution::P480, bad_duration)).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));

    let err = session.job_status_batch(&[]).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  // ── Live (ignored: needs captured cookies; the enqueue spends credits) ──

  #[tokio::test]
  #[ignore]
  async fn live_session_user_data() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_session;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_test_session()?;
    let user = session.user_data().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    let profile = session.user_profile().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("User {} ({}) plan={:?} credits={:?}", user.id, profile.username, user.plan_type, user.subscription_credits);
    Ok(())
  }

  #[tokio::test]
  #[ignore]
  async fn live_session_generate_and_wait() -> anyhow::Result<()> {
    use crate::session::wait_for_job::WaitForJobOptions;
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_session;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_test_session()?;
    let enqueued = session.nano_banana_pro(
      NanoBananaProRequest::text_to_image("a dinosaur on a skateboard", NanoBananaProAspectRatio::Portrait3x4, NanoBananaProResolution::OneK),
    ).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    let job_id = enqueued.job_ids().into_iter().next().unwrap();
    let job = session.wait_for_job(&job_id, WaitForJobOptions::default()).await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("Job {} finished: {:?}", job.id, job.result_url());
    assert!(job.result_url().is_some());
    Ok(())
  }
}
