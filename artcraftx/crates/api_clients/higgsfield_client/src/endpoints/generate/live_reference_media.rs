//! Live, credit-spending tests for reference media: upload the fixtures in
//! `test_assets/` through [`HiggsfieldSession`], then feed them to the
//! bindings as reference images, start / end frames, and (upload only)
//! video and audio. Each job is followed through the status endpoints and
//! its echoed `params.medias` / `params.input_images` checked.
//!
//! ```text
//! cargo test -p higgsfield_client live_reference -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Cheapest settings throughout; the upload-only test is free.

use crate::endpoints::generate::image::gpt_image_2::{GptImage2AspectRatio, GptImage2Quality, GptImage2Request, GptImage2Resolution};
use crate::endpoints::generate::image::nano_banana_2::{NanoBanana2Request, NanoBanana2Resolution};
use crate::endpoints::generate::image::nano_banana_2_lite::{NanoBanana2LiteQuality, NanoBanana2LiteRequest};
use crate::endpoints::generate::image::nano_banana_pro::{NanoBananaProRequest, NanoBananaProResolution};
use crate::endpoints::generate::image::seedream_4p5::{Seedream4p5Request, Seedream4p5Resolution};
use crate::endpoints::generate::image::seedream_5p0_lite::{Seedream5p0LiteRequest, Seedream5p0LiteResolution};
use crate::endpoints::generate::image::seedream_5p0_pro::{Seedream5p0ProRequest, Seedream5p0ProResolution};
use crate::endpoints::generate::video::grok_imagine_1p5::{GrokImagine1p5Request, GrokImagine1p5Resolution};
use crate::endpoints::generate::video::kling_3p0::{Kling3p0Request, Kling3p0Resolution};
use crate::endpoints::generate::video::minimax_h3::MinimaxH3Request;
use crate::endpoints::generate::video::seedance_2p0::{Seedance2p0Request, Seedance2p0Resolution};
use crate::endpoints::generate::video::seedance_2p0_mini::{Seedance2p0MiniRequest, Seedance2p0MiniResolution};
use crate::endpoints::generate::video::seedance_2p5::{Seedance2p5Request, Seedance2p5Resolution};
use crate::session::higgsfield_session::HiggsfieldSession;
use crate::session::upload_media::ReferenceMediaFile;
use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_session;
use crate::test_utils::poll_job_to_completion::poll_jobs_to_completion;
use crate::test_utils::setup_test_logging::setup_test_logging;
use crate::types::enqueue_jobs_response::EnqueueJobsResponse;
use crate::types::job_set_type::JobSetType;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::session::wait_for_job::WaitForJobOptions;
use crate::types::media_input::{MediaInput, MediaInputKind};
use crate::types::media_mime_type::MediaMimeType;
use crate::types::media_reference::MediaReference;
use crate::types::media_role::MediaRole;
use crate::types::nano_banana_aspect_ratio::NanoBananaAspectRatio;
use crate::types::seedream_aspect_ratio::SeedreamAspectRatio;
use crate::types::video_aspect_ratio::{KlingAspectRatio, SeedanceVideoAspectRatio};
use crate::types::video_dimensions::VideoDimensions;
use crate::types::video_duration::VideoDurationSeconds;

/// 640x640 PNG: teal background, orange disc. Over GPT Image 2's 300px
/// minimum.
const REFERENCE_PNG: &[u8] = include_bytes!("../../../test_assets/reference_640.png");

/// 4s, 8kHz mono 440Hz tone with a slow tremolo. (A 1s tone was rejected
/// by Seedance: "Input audio duration is not supported".)
const TONE_WAV: &[u8] = include_bytes!("../../../test_assets/tone_4s.wav");

/// 4s, 256x256 H.264 clip: an orange square sliding over teal.
const CLIP_MP4: &[u8] = include_bytes!("../../../test_assets/clip_4s.mp4");

const PROMPT_IMAGE: &str = "the orange disc from the reference, but as a shiba inu's face, flat vector style";
const PROMPT_VIDEO: &str = "the orange disc rolls off screen to the right";

fn session() -> anyhow::Result<HiggsfieldSession> {
  setup_test_logging();
  load_higgsfield_test_session()
}

async fn upload_png(session: &HiggsfieldSession, name: &str) -> anyhow::Result<MediaInput> {
  let input = session.upload_reference_media(ReferenceMediaFile::new(name, MediaMimeType::ImagePng, REFERENCE_PNG.to_vec()))
      .await.map_err(|err| anyhow::anyhow!("{err}"))?;
  println!("uploaded {name} => id={} url={}", input.id, input.url);
  Ok(input)
}

/// Seedance refuses media whose IP check hasn't run; upload with it.
async fn upload_png_ip_checked(session: &HiggsfieldSession, name: &str) -> anyhow::Result<MediaInput> {
  let input = session.upload_reference_media(ReferenceMediaFile::new(name, MediaMimeType::ImagePng, REFERENCE_PNG.to_vec()).with_ip_check())
      .await.map_err(|err| anyhow::anyhow!("{err}"))?;
  println!("uploaded {name} (IP checked) => id={} url={}", input.id, input.url);
  Ok(input)
}

/// Follow the job(s) and check the server echoed our references back.
async fn follow(session: &HiggsfieldSession, label: &str, expected_type: JobSetType, expected_roles: &[MediaRole], expected_input_images: usize, response: EnqueueJobsResponse) -> anyhow::Result<()> {
  let job_set = response.first_job_set().ok_or_else(|| anyhow::anyhow!("no job set"))?;
  println!(
    "\n##### {label}: enqueued job set {} type={} cost={:?} medias={:?} input_images={}",
    job_set.id, job_set.job_set_type, job_set.cost,
    job_set.params.medias.iter().map(|m| m.role.as_str()).collect::<Vec<_>>(), job_set.params.input_images.len(),
  );
  assert_eq!(job_set.job_set_type, expected_type);
  let echoed_roles: Vec<MediaRole> = job_set.params.medias.iter().map(|m| m.role.clone()).collect();
  assert_eq!(echoed_roles, expected_roles, "server should echo the reference roles");
  assert_eq!(job_set.params.input_images.len(), expected_input_images);

  let jobs = poll_jobs_to_completion(session, &response.job_ids()).await?;
  for job in &jobs {
    assert!(job.result_url().is_some(), "job {} has no result url", job.id);
    assert_eq!(job.params.medias.len(), expected_roles.len());
    assert_eq!(job.params.input_images.len(), expected_input_images);
  }
  println!("##### {label}: done ({} job(s))", jobs.len());
  Ok(())
}

// ── Upload only (free) ──

#[tokio::test]
#[ignore]
async fn live_reference_upload_png_single_and_batch_with_video_and_audio() -> anyhow::Result<()> {
  let session = session()?;

  let single = upload_png(&session, "reference_640.png").await?;
  assert!(single.url.ends_with(".png"));

  let batch = session.upload_reference_media_batch(vec![
    ReferenceMediaFile::new("start_frame.png", MediaMimeType::ImagePng, REFERENCE_PNG.to_vec()),
    ReferenceMediaFile::new("end_frame.png", MediaMimeType::ImagePng, REFERENCE_PNG.to_vec()),
  ]).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  for input in &batch {
    println!("batch uploaded => id={} url={}", input.id, input.url);
  }
  assert_eq!(batch.len(), 2);
  assert!(batch.iter().all(|input| input.url.ends_with(".png")));

  // Video and audio go through the single-file presign.
  let clip = session.upload_reference_media(ReferenceMediaFile::new("clip_4s.mp4", MediaMimeType::VideoMp4, CLIP_MP4.to_vec()))
      .await.map_err(|err| anyhow::anyhow!("{err}"))?;
  println!("uploaded clip_1s.mp4 => id={} url={}", clip.id, clip.url);
  assert!(clip.url.ends_with(".mp4"));

  let tone = session.upload_reference_media(ReferenceMediaFile::new("tone_4s.wav", MediaMimeType::AudioWav, TONE_WAV.to_vec()))
      .await.map_err(|err| anyhow::anyhow!("{err}"))?;
  println!("uploaded tone_1s.wav => id={} url={}", tone.id, tone.url);
  assert!(tone.url.ends_with(".wav"));
  Ok(())
}

// ── Image models ──

#[tokio::test]
#[ignore]
async fn live_reference_nano_banana_2_lite_image_ref() -> anyhow::Result<()> {
  let session = session()?;
  let reference = upload_png(&session, "reference_640.png").await?;
  let request = NanoBanana2LiteRequest::text_to_image(PROMPT_IMAGE, NanoBananaAspectRatio::Square1x1, NanoBanana2LiteQuality::Minimal)
      .with_reference_images(vec![reference]);
  let response = session.nano_banana_2_lite(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Nano Banana 2 Lite + reference", JobSetType::NanoBanana2Lite, &[MediaRole::Image], 0, response).await
}

#[tokio::test]
#[ignore]
async fn live_reference_nano_banana_2_image_ref() -> anyhow::Result<()> {
  let session = session()?;
  let reference = upload_png(&session, "reference_640.png").await?;
  let request = NanoBanana2Request::text_to_image(PROMPT_IMAGE, NanoBananaAspectRatio::Square1x1, NanoBanana2Resolution::OneK)
      .with_reference_images(vec![reference]);
  let response = session.nano_banana_2(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Nano Banana 2 + reference", JobSetType::NanoBananaFlash, &[MediaRole::Image], 0, response).await
}

#[tokio::test]
#[ignore]
async fn live_reference_nano_banana_pro_input_image() -> anyhow::Result<()> {
  let session = session()?;
  let reference = upload_png(&session, "reference_640.png").await?;
  let request = NanoBananaProRequest::text_to_image(PROMPT_IMAGE, NanoBananaAspectRatio::Square1x1, NanoBananaProResolution::OneK)
      .with_reference_images(vec![reference]);
  let response = session.nano_banana_pro(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Nano Banana Pro + input image", JobSetType::NanoBanana2, &[], 1, response).await
}

#[tokio::test]
#[ignore]
async fn live_reference_gpt_image_2_image_ref() -> anyhow::Result<()> {
  let session = session()?;
  let reference = upload_png(&session, "reference_640.png").await?;
  let request = GptImage2Request::text_to_image(PROMPT_IMAGE, GptImage2AspectRatio::Auto, GptImage2Quality::Low, GptImage2Resolution::OneK)
      .with_reference_images(vec![reference]);
  let response = session.gpt_image_2(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "GPT Image 2 + reference", JobSetType::GptImage2, &[MediaRole::Image], 0, response).await
}

#[tokio::test]
#[ignore]
async fn live_reference_seedream_5p0_pro_image_ref() -> anyhow::Result<()> {
  let session = session()?;
  let reference = upload_png(&session, "reference_640.png").await?;
  let request = Seedream5p0ProRequest::text_to_image(PROMPT_IMAGE, SeedreamAspectRatio::Square1x1, Seedream5p0ProResolution::OneK)
      .with_reference_images(vec![reference]);
  let response = session.seedream_5p0_pro(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Seedream 5.0 Pro + reference", JobSetType::SeedreamV5Pro, &[MediaRole::Image], 0, response).await
}

#[tokio::test]
#[ignore]
async fn live_reference_seedream_5p0_lite_image_ref() -> anyhow::Result<()> {
  let session = session()?;
  let reference = upload_png(&session, "reference_640.png").await?;
  let request = Seedream5p0LiteRequest::text_to_image(PROMPT_IMAGE, SeedreamAspectRatio::Square1x1, Seedream5p0LiteResolution::TwoK)
      .with_reference_images(vec![reference]);
  let response = session.seedream_5p0_lite(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Seedream 5.0 lite + reference", JobSetType::SeedreamV5Lite, &[MediaRole::Image], 0, response).await
}

#[tokio::test]
#[ignore]
async fn live_reference_seedream_4p5_input_image() -> anyhow::Result<()> {
  let session = session()?;
  let reference = upload_png(&session, "reference_640.png").await?;
  let request = Seedream4p5Request::text_to_image(PROMPT_IMAGE, SeedreamAspectRatio::Square1x1, Seedream4p5Resolution::TwoK)
      .with_reference_images(vec![reference]);
  let response = session.seedream_4p5(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Seedream 4.5 + input image", JobSetType::SeedreamV4p5, &[], 1, response).await
}

// ── Video models ──

#[tokio::test]
#[ignore]
async fn live_reference_grok_imagine_1p5_start_frame() -> anyhow::Result<()> {
  let session = session()?;
  let frame = upload_png(&session, "start_frame.png").await?;
  let request = GrokImagine1p5Request::text_to_video(PROMPT_VIDEO, GrokImagine1p5Resolution::P480, VideoDurationSeconds::new(1))
      .with_media(MediaReference::start_frame(frame));
  let response = session.grok_imagine_1p5(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Grok Imagine 1.5 + start frame", JobSetType::GrokVideoV15, &[MediaRole::StartImage], 0, response).await
}

#[tokio::test]
#[ignore]
async fn live_reference_seedance_2p0_mini_image_ref() -> anyhow::Result<()> {
  let session = session()?;
  let reference = upload_png_ip_checked(&session, "reference_640.png").await?;
  let request = Seedance2p0MiniRequest::text_to_video(PROMPT_VIDEO, SeedanceVideoAspectRatio::Square1x1, Seedance2p0MiniResolution::P480, VideoDurationSeconds::new(4))
      .with_media(MediaReference::image(reference));
  let response = session.seedance_2p0_mini(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Seedance 2.0 Mini + reference image", JobSetType::Seedance2p0Mini, &[MediaRole::Image], 0, response).await
}

#[tokio::test]
#[ignore]
async fn live_reference_seedance_2p0_mini_start_and_end_frames() -> anyhow::Result<()> {
  let session = session()?;
  let frames = session.upload_reference_media_batch(vec![
    ReferenceMediaFile::new("start_frame.png", MediaMimeType::ImagePng, REFERENCE_PNG.to_vec()).with_ip_check(),
    ReferenceMediaFile::new("end_frame.png", MediaMimeType::ImagePng, REFERENCE_PNG.to_vec()).with_ip_check(),
  ]).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  let request = Seedance2p0MiniRequest::text_to_video(PROMPT_VIDEO, SeedanceVideoAspectRatio::Square1x1, Seedance2p0MiniResolution::P480, VideoDurationSeconds::new(4))
      .with_media(MediaReference::start_frame(frames[0].clone()))
      .with_media(MediaReference::end_frame(frames[1].clone()));
  let response = session.seedance_2p0_mini(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Seedance 2.0 Mini + frames", JobSetType::Seedance2p0Mini, &[MediaRole::StartImage, MediaRole::EndImage], 0, response).await
}

/// Video + audio references (roles from the web app's bundle, not a
/// browser capture): the cheapest model that takes them.
///
/// The wire contract is what this asserts: the enqueue is accepted with
/// `video_input` / `audio_input` descriptors and the roles are echoed. The
/// job itself has failed at the model stage on the synthetic fixtures
/// (2026-08-31: "Input audio duration is not supported" for a 1s tone,
/// then a generic "change your input files" for the 4s clip + tone), so
/// a model-stage failure is reported, not treated as a client bug. Swap
/// in real footage / a real track to see it complete.
#[tokio::test]
#[ignore]
async fn live_reference_seedance_2p0_mini_video_and_audio_refs() -> anyhow::Result<()> {
  let session = session()?;
  let clip = session.upload_reference_media(ReferenceMediaFile::new("clip_4s.mp4", MediaMimeType::VideoMp4, CLIP_MP4.to_vec()).with_ip_check())
      .await.map_err(|err| anyhow::anyhow!("{err}"))?;
  let tone = session.upload_reference_media(ReferenceMediaFile::new("tone_4s.wav", MediaMimeType::AudioWav, TONE_WAV.to_vec()).with_ip_check())
      .await.map_err(|err| anyhow::anyhow!("{err}"))?;
  let request = Seedance2p0MiniRequest::text_to_video(PROMPT_VIDEO, SeedanceVideoAspectRatio::Square1x1, Seedance2p0MiniResolution::P480, VideoDurationSeconds::new(4))
      .with_media(MediaReference::video(clip))
      .with_media(MediaReference::audio(tone));
  let response = session.seedance_2p0_mini(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  let job_set = response.first_job_set().ok_or_else(|| anyhow::anyhow!("no job set"))?;
  let echoed: Vec<(MediaRole, MediaInputKind)> = job_set.params.medias.iter().map(|m| (m.role.clone(), m.data.kind.clone())).collect();
  println!("\n##### Seedance 2.0 Mini + video + audio: enqueued job set {} cost={:?} medias={:?}", job_set.id, job_set.cost, echoed);
  assert_eq!(echoed, vec![(MediaRole::Video, MediaInputKind::VideoInput), (MediaRole::Audio, MediaInputKind::AudioInput)]);

  let job_id = response.job_ids().into_iter().next().unwrap();
  match session.wait_for_job(&job_id, WaitForJobOptions::default()).await {
    Ok(job) => println!("##### completed: {:?}", job.result_url()),
    Err(HiggsfieldError::Client(HiggsfieldClientError::JobFailed { maybe_reason: Some(reason), .. })) =>
      println!("##### model-stage failure on the synthetic fixtures (not a wire problem): {reason}"),
    Err(err) => return Err(anyhow::anyhow!("{err}")),
  }
  Ok(())
}

#[tokio::test]
#[ignore]
async fn live_reference_seedance_2p0_image_ref() -> anyhow::Result<()> {
  let session = session()?;
  let reference = upload_png_ip_checked(&session, "reference_640.png").await?;
  let request = Seedance2p0Request::text_to_video(PROMPT_VIDEO, SeedanceVideoAspectRatio::Square1x1, Seedance2p0Resolution::P480, VideoDurationSeconds::new(4))
      .with_media(MediaReference::image(reference));
  let response = session.seedance_2p0(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Seedance 2.0 + reference image", JobSetType::Seedance2p0, &[MediaRole::Image], 0, response).await
}

#[tokio::test]
#[ignore]
async fn live_reference_seedance_2p5_image_ref() -> anyhow::Result<()> {
  let session = session()?;
  let reference = upload_png_ip_checked(&session, "reference_640.png").await?;
  let request = Seedance2p5Request::text_to_video(PROMPT_VIDEO, SeedanceVideoAspectRatio::Square1x1, Seedance2p5Resolution::P480, VideoDurationSeconds::new(4))
      .with_media(MediaReference::image(reference));
  let response = session.seedance_2p5(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Seedance 2.5 + reference image", JobSetType::Seedance2p5, &[MediaRole::Image], 0, response).await
}

#[tokio::test]
#[ignore]
async fn live_reference_kling_3p0_start_and_end_frames() -> anyhow::Result<()> {
  let session = session()?;
  let frames = session.upload_reference_media_batch(vec![
    ReferenceMediaFile::new("start_frame.png", MediaMimeType::ImagePng, REFERENCE_PNG.to_vec()),
    ReferenceMediaFile::new("end_frame.png", MediaMimeType::ImagePng, REFERENCE_PNG.to_vec()),
  ]).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  let request = Kling3p0Request::text_to_video(PROMPT_VIDEO, KlingAspectRatio::Square1x1, Kling3p0Resolution::P720, VideoDurationSeconds::new(3))
      .with_media(MediaReference::start_frame(frames[0].clone()))
      .with_media(MediaReference::end_frame(frames[1].clone()));
  let response = session.kling_3p0(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "Kling 3.0 + frames", JobSetType::Kling3p0, &[MediaRole::StartImage, MediaRole::EndImage], 0, response).await
}

#[tokio::test]
#[ignore]
async fn live_reference_minimax_h3_start_and_end_frames() -> anyhow::Result<()> {
  let session = session()?;
  let frames = session.upload_reference_media_batch(vec![
    ReferenceMediaFile::new("start_frame.png", MediaMimeType::ImagePng, REFERENCE_PNG.to_vec()),
    ReferenceMediaFile::new("end_frame.png", MediaMimeType::ImagePng, REFERENCE_PNG.to_vec()),
  ]).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  let mut request = MinimaxH3Request::text_to_video(PROMPT_VIDEO, VideoDurationSeconds::new(5))
      .with_media(MediaReference::start_frame(frames[0].clone()))
      .with_media(MediaReference::end_frame(frames[1].clone()));
  // Square frames: the web app sends a square 2K canvas.
  request.maybe_dimensions = Some(VideoDimensions::new(2048, 2048));
  let response = session.minimax_h3(request).await.map_err(|err| anyhow::anyhow!("{err}"))?;
  follow(&session, "MiniMax H3 + frames", JobSetType::MinimaxH3, &[MediaRole::StartImage, MediaRole::EndImage], 0, response).await
}
