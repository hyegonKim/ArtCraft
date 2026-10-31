//! Live, credit-spending image-to-video matrix over real photos: a
//! character (a red panda), a living room with a Christmas tree, and a
//! 1980s mall sign, run through every Seedance / MiniMax binding in each
//! of the three image-to-video modalities:
//!
//! 1. references — all three photos as role `image`, a prompt that ties
//!    them together;
//! 2. start frame — the character as `start_image`, nothing else;
//! 3. keyframes — the character as `start_image`, the mall as `end_image`.
//!
//! Seedance 2.5 Edit is video-to-video: it runs last, editing the Seedance
//! 2.5 start-frame result with the three photos as references.
//!
//! The photos live outside the repo in `external/test_media/` (see its
//! `SOURCES.md`; real photographs only — synthetic test images trip the
//! service's filters). The character defaults to `red_panda.jpg`; set
//! `HIGGSFIELD_I2V_CHARACTER` to another file name in that directory.
//! (Photos of Jim Varney as Ernest were the original character; Higgsfield's
//! IP check flags them as protected content — `ip_detected`, then `404
//! Media input not found` on enqueue — so they can't be used.)
//! Each job's outcome is appended to
//! `external/test_media/outputs/results.tsv` (`model  modality  job_id
//! status  url`), so the clips can be pulled down afterwards.
//!
//! ```text
//! cargo test -p higgsfield_client live_i2v -- --ignored --nocapture --test-threads=1
//! ```

use crate::endpoints::generate::video::minimax_h3::MinimaxH3Request;
use crate::endpoints::generate::video::seedance_2p0::{Seedance2p0Request, Seedance2p0Resolution};
use crate::endpoints::generate::video::seedance_2p0_mini::{Seedance2p0MiniRequest, Seedance2p0MiniResolution};
use crate::endpoints::generate::video::seedance_2p5::{Seedance2p5Request, Seedance2p5Resolution};
use crate::endpoints::generate::video::seedance_2p5_edit::Seedance2p5EditRequest;
use crate::endpoints::jobs::job_status::JobStatusResponse;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::session::higgsfield_session::HiggsfieldSession;
use crate::session::upload_media::ReferenceMediaFile;
use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_session;
use crate::test_utils::poll_job_to_completion::poll_job_to_completion;
use crate::test_utils::setup_test_logging::setup_test_logging;
use crate::types::enqueue_jobs_response::EnqueueJobsResponse;
use crate::types::media_input::MediaInput;
use crate::types::media_reference::MediaReference;
use crate::types::media_role::MediaRole;
use crate::types::video_aspect_ratio::SeedanceVideoAspectRatio;
use crate::types::video_duration::VideoDurationSeconds;
use std::fs::{read, read_to_string, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

const MEDIA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../external/test_media");

const PROMPT_REFERENCES: &str = "The red panda from the first image is sitting in the cozy brick living room from the second image, right beside the lit Christmas tree. It bats a red ornament with its paw, then looks straight at the camera; through the window behind it the sunlit 1980s Sarasota Square shopping mall sign and palm trees from the third image are visible.";

const PROMPT_START_FRAME: &str = "The red panda on the branch turns its head to the camera, yawns wide, stretches, then scampers along the branch and out of frame to the right.";

const PROMPT_KEYFRAMES: &str = "The red panda hops off the branch and trots to the right; the camera follows it out into bright Florida sunshine and cranes up to reveal the Sarasota Square shopping mall sign standing among tall palm trees.";

const PROMPT_EDIT: &str = "Keep the red panda exactly as it is, but replace the background with the cozy brick living room and lit Christmas tree from the reference photo, and put the sunlit Sarasota Square mall sign with palm trees outside the window behind it.";

/// The three uploaded photos.
struct Photos {
  character: MediaInput,
  tree: MediaInput,
  mall: MediaInput,
}

impl Photos {
  fn references(&self) -> [MediaReference; 3] {
    [MediaReference::image(self.character.clone()), MediaReference::image(self.tree.clone()), MediaReference::image(self.mall.clone())]
  }
}

fn session() -> anyhow::Result<HiggsfieldSession> {
  setup_test_logging();
  load_higgsfield_test_session()
}

fn media_path(name: &str) -> PathBuf {
  PathBuf::from(MEDIA_DIR).join(name)
}

/// Upload the photos with the IP check (Seedance insists on it).
async fn upload_photos(session: &HiggsfieldSession) -> anyhow::Result<Photos> {
  let character = std::env::var("HIGGSFIELD_I2V_CHARACTER").unwrap_or_else(|_| "red_panda.jpg".to_string());
  let mut uploaded = Vec::with_capacity(3);
  for name in [character.as_str(), "christmas_tree.jpg", "mall_1980s.jpg"] {
    let bytes = read(media_path(name)).map_err(|err| anyhow::anyhow!("read {}: {err} (see external/test_media/SOURCES.md)", media_path(name).display()))?;
    let input = session.upload_reference_media(ReferenceMediaFile::from_file_name(name, bytes)?.with_ip_check())
        .await.map_err(|err| anyhow::anyhow!("{name}: {err}"))?;
    println!("uploaded {name} => {} {}", input.id, input.url);
    uploaded.push(input);
  }
  let mall = uploaded.pop().unwrap();
  let tree = uploaded.pop().unwrap();
  let character = uploaded.pop().unwrap();
  Ok(Photos { character, tree, mall })
}

/// Seedance's catch-all failure. Seen on jobs that succeeded verbatim a
/// few minutes later, so it's treated as transient.
const GENERIC_FAILURE: &str = "Something went wrong";

type EnqueueFuture<'a> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<EnqueueJobsResponse, HiggsfieldError>> + Send + 'a>>;

/// Enqueue, follow the job, check the echoed roles, and record the outcome.
/// On the generic model-stage failure the session token is refreshed and
/// the same request enqueued once more (credits for failed jobs are
/// refunded), and both attempts are recorded — so the log shows whether a
/// fresh session is what fixes it.
async fn run<F>(session: &HiggsfieldSession, model: &str, modality: &str, expected_roles: &[MediaRole], enqueue: F) -> anyhow::Result<JobStatusResponse>
where
  F: for<'a> Fn(&'a HiggsfieldSession) -> EnqueueFuture<'a>,
{
  for attempt in 1..=2 {
    let response = enqueue(session).await.map_err(|err| anyhow::anyhow!("{err}"))?;
    let job_set = response.first_job_set().ok_or_else(|| anyhow::anyhow!("no job set"))?;
    let echoed: Vec<MediaRole> = job_set.params.medias.iter().map(|m| m.role.clone()).collect();
    println!(
      "\n##### {model} / {modality} (attempt {attempt}): job set {} cost={:?} server size={:?}x{:?} aspect={:?} medias={:?}",
      job_set.id, job_set.cost, job_set.params.width, job_set.params.height, job_set.params.aspect_ratio, echoed,
    );
    assert_eq!(echoed, expected_roles, "server should echo the reference roles");

    let job_id = response.job_ids().into_iter().next().ok_or_else(|| anyhow::anyhow!("no job"))?;
    match poll_job_to_completion(session, &job_id).await {
      Ok(job) => {
        record(model, modality, job_id.as_str(), &format!("completed (attempt {attempt})"), job.result_url().unwrap_or(""));
        return Ok(job);
      }
      Err(err) => {
        let reason = err.to_string().replace(['\t', '\n'], " ");
        record(model, modality, job_id.as_str(), &format!("failed (attempt {attempt})"), &reason);
        if attempt == 1 && reason.contains(GENERIC_FAILURE) {
          let token = session.refresh().await.map_err(|err| anyhow::anyhow!("{err}"))?;
          println!("##### {model} / {modality}: generic failure; refreshed session token (expires {}) and retrying once", token.expires_at());
          continue;
        }
        return Err(err);
      }
    }
  }
  unreachable!("the loop returns on the second attempt")
}

fn record(model: &str, modality: &str, job_id: &str, status: &str, url: &str) {
  let path = media_path("outputs/results.tsv");
  if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
    let _ = writeln!(file, "{model}\t{modality}\t{job_id}\t{status}\t{url}");
  }
}

/// The Seedance 2.5 start-frame clip, as recorded by an earlier test in
/// this file, for the edit test to work on.
fn recorded_edit_source() -> anyhow::Result<MediaInput> {
  let tsv = read_to_string(media_path("outputs/results.tsv"))?;
  for line in tsv.lines().rev() {
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() == 5 && fields[0] == "seedance_2p5" && fields[1] == "start_frame" && fields[3].starts_with("completed") {
      return Ok(MediaInput::from_video_job(fields[2], fields[4]));
    }
  }
  anyhow::bail!("no completed seedance_2p5 start_frame job in outputs/results.tsv; run live_i2v_seedance_2p5_2_start_frame first")
}

const FOUR_SECONDS: VideoDurationSeconds = VideoDurationSeconds::new(4);
const FIVE_SECONDS: VideoDurationSeconds = VideoDurationSeconds::new(5);

// ── Seedance 2.5 ──

#[tokio::test]
#[ignore]
async fn live_i2v_seedance_2p5_1_references() -> anyhow::Result<()> {
  let session = session()?;
  let photos = upload_photos(&session).await?;
  let mut request = Seedance2p5Request::text_to_video(PROMPT_REFERENCES, SeedanceVideoAspectRatio::Landscape16x9, Seedance2p5Resolution::P480, FOUR_SECONDS);
  request.medias = photos.references().to_vec();
  run(&session, "seedance_2p5", "references", &[MediaRole::Image, MediaRole::Image, MediaRole::Image], |session| { let request = request.clone(); Box::pin(async move { session.seedance_2p5(request).await }) }).await?;
  Ok(())
}

#[tokio::test]
#[ignore]
async fn live_i2v_seedance_2p5_2_start_frame() -> anyhow::Result<()> {
  let session = session()?;
  let photos = upload_photos(&session).await?;
  let request = Seedance2p5Request::text_to_video(PROMPT_START_FRAME, SeedanceVideoAspectRatio::Landscape4x3, Seedance2p5Resolution::P480, FOUR_SECONDS)
      .with_media(MediaReference::start_frame(photos.character));
  run(&session, "seedance_2p5", "start_frame", &[MediaRole::StartImage], |session| { let request = request.clone(); Box::pin(async move { session.seedance_2p5(request).await }) }).await?;
  Ok(())
}

#[tokio::test]
#[ignore]
async fn live_i2v_seedance_2p5_3_keyframes() -> anyhow::Result<()> {
  let session = session()?;
  let photos = upload_photos(&session).await?;
  let request = Seedance2p5Request::text_to_video(PROMPT_KEYFRAMES, SeedanceVideoAspectRatio::Landscape4x3, Seedance2p5Resolution::P480, FOUR_SECONDS)
      .with_media(MediaReference::start_frame(photos.character))
      .with_media(MediaReference::end_frame(photos.mall));
  run(&session, "seedance_2p5", "keyframes", &[MediaRole::StartImage, MediaRole::EndImage], |session| { let request = request.clone(); Box::pin(async move { session.seedance_2p5(request).await }) }).await?;
  Ok(())
}

// ── Seedance 2.0 ──

#[tokio::test]
#[ignore]
async fn live_i2v_seedance_2p0_1_references() -> anyhow::Result<()> {
  let session = session()?;
  let photos = upload_photos(&session).await?;
  let mut request = Seedance2p0Request::text_to_video(PROMPT_REFERENCES, SeedanceVideoAspectRatio::Landscape16x9, Seedance2p0Resolution::P480, FOUR_SECONDS);
  request.medias = photos.references().to_vec();
  run(&session, "seedance_2p0", "references", &[MediaRole::Image, MediaRole::Image, MediaRole::Image], |session| { let request = request.clone(); Box::pin(async move { session.seedance_2p0(request).await }) }).await?;
  Ok(())
}

#[tokio::test]
#[ignore]
async fn live_i2v_seedance_2p0_2_start_frame() -> anyhow::Result<()> {
  let session = session()?;
  let photos = upload_photos(&session).await?;
  let request = Seedance2p0Request::text_to_video(PROMPT_START_FRAME, SeedanceVideoAspectRatio::Landscape4x3, Seedance2p0Resolution::P480, FOUR_SECONDS)
      .with_media(MediaReference::start_frame(photos.character));
  run(&session, "seedance_2p0", "start_frame", &[MediaRole::StartImage], |session| { let request = request.clone(); Box::pin(async move { session.seedance_2p0(request).await }) }).await?;
  Ok(())
}

#[tokio::test]
#[ignore]
async fn live_i2v_seedance_2p0_3_keyframes() -> anyhow::Result<()> {
  let session = session()?;
  let photos = upload_photos(&session).await?;
  let request = Seedance2p0Request::text_to_video(PROMPT_KEYFRAMES, SeedanceVideoAspectRatio::Landscape4x3, Seedance2p0Resolution::P480, FOUR_SECONDS)
      .with_media(MediaReference::start_frame(photos.character))
      .with_media(MediaReference::end_frame(photos.mall));
  run(&session, "seedance_2p0", "keyframes", &[MediaRole::StartImage, MediaRole::EndImage], |session| { let request = request.clone(); Box::pin(async move { session.seedance_2p0(request).await }) }).await?;
  Ok(())
}

// ── Seedance 2.0 Mini ──

#[tokio::test]
#[ignore]
async fn live_i2v_seedance_2p0_mini_1_references() -> anyhow::Result<()> {
  let session = session()?;
  let photos = upload_photos(&session).await?;
  let mut request = Seedance2p0MiniRequest::text_to_video(PROMPT_REFERENCES, SeedanceVideoAspectRatio::Landscape16x9, Seedance2p0MiniResolution::P480, FOUR_SECONDS);
  request.medias = photos.references().to_vec();
  run(&session, "seedance_2p0_mini", "references", &[MediaRole::Image, MediaRole::Image, MediaRole::Image], |session| { let request = request.clone(); Box::pin(async move { session.seedance_2p0_mini(request).await }) }).await?;
  Ok(())
}

#[tokio::test]
#[ignore]
async fn live_i2v_seedance_2p0_mini_2_start_frame() -> anyhow::Result<()> {
  let session = session()?;
  let photos = upload_photos(&session).await?;
  let request = Seedance2p0MiniRequest::text_to_video(PROMPT_START_FRAME, SeedanceVideoAspectRatio::Landscape4x3, Seedance2p0MiniResolution::P480, FOUR_SECONDS)
      .with_media(MediaReference::start_frame(photos.character));
  run(&session, "seedance_2p0_mini", "start_frame", &[MediaRole::StartImage], |session| { let request = request.clone(); Box::pin(async move { session.seedance_2p0_mini(request).await }) }).await?;
  Ok(())
}

#[tokio::test]
#[ignore]
async fn live_i2v_seedance_2p0_mini_3_keyframes() -> anyhow::Result<()> {
  let session = session()?;
  let photos = upload_photos(&session).await?;
  let request = Seedance2p0MiniRequest::text_to_video(PROMPT_KEYFRAMES, SeedanceVideoAspectRatio::Landscape4x3, Seedance2p0MiniResolution::P480, FOUR_SECONDS)
      .with_media(MediaReference::start_frame(photos.character))
      .with_media(MediaReference::end_frame(photos.mall));
  run(&session, "seedance_2p0_mini", "keyframes", &[MediaRole::StartImage, MediaRole::EndImage], |session| { let request = request.clone(); Box::pin(async move { session.seedance_2p0_mini(request).await }) }).await?;
  Ok(())
}

// ── MiniMax H3 ──

#[tokio::test]
#[ignore]
async fn live_i2v_minimax_h3_1_references() -> anyhow::Result<()> {
  let session = session()?;
  let photos = upload_photos(&session).await?;
  let mut request = MinimaxH3Request::text_to_video(PROMPT_REFERENCES, FIVE_SECONDS);
  request.medias = photos.references().to_vec();
  run(&session, "minimax_h3", "references", &[MediaRole::Image, MediaRole::Image, MediaRole::Image], |session| { let request = request.clone(); Box::pin(async move { session.minimax_h3(request).await }) }).await?;
  Ok(())
}

#[tokio::test]
#[ignore]
async fn live_i2v_minimax_h3_2_start_frame() -> anyhow::Result<()> {
  let session = session()?;
  let photos = upload_photos(&session).await?;
  let request = MinimaxH3Request::text_to_video(PROMPT_START_FRAME, FIVE_SECONDS)
      .with_media(MediaReference::start_frame(photos.character));
  run(&session, "minimax_h3", "start_frame", &[MediaRole::StartImage], |session| { let request = request.clone(); Box::pin(async move { session.minimax_h3(request).await }) }).await?;
  Ok(())
}

#[tokio::test]
#[ignore]
async fn live_i2v_minimax_h3_3_keyframes() -> anyhow::Result<()> {
  let session = session()?;
  let photos = upload_photos(&session).await?;
  let request = MinimaxH3Request::text_to_video(PROMPT_KEYFRAMES, FIVE_SECONDS)
      .with_media(MediaReference::start_frame(photos.character))
      .with_media(MediaReference::end_frame(photos.mall));
  run(&session, "minimax_h3", "keyframes", &[MediaRole::StartImage, MediaRole::EndImage], |session| { let request = request.clone(); Box::pin(async move { session.minimax_h3(request).await }) }).await?;
  Ok(())
}

// ── Seedance 2.5 Edit (video-to-video; runs last) ──

#[tokio::test]
#[ignore]
async fn live_i2v_z_seedance_2p5_edit_references() -> anyhow::Result<()> {
  let session = session()?;
  let source = recorded_edit_source()?;
  println!("editing {} ({})", source.id, source.url);
  let photos = upload_photos(&session).await?;
  let mut request = Seedance2p5EditRequest::new(PROMPT_EDIT, source, Seedance2p5Resolution::P480);
  request.references = photos.references().to_vec();
  run(&session, "seedance_2p5_edit", "references", &[MediaRole::Video, MediaRole::Image, MediaRole::Image, MediaRole::Image], |session| { let request = request.clone(); Box::pin(async move { session.seedance_2p5_edit(request).await }) }).await?;
  Ok(())
}
