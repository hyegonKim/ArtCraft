//! Planning helpers shared by every Higgsfield video model. The resolution
//! and aspect-ratio snapping primitives come from the image provider's
//! `common`; this adds durations, batches, bitrates, and reference triage.

use higgsfield_client::types::image_batch_size::ImageBatchSize;
use higgsfield_client::types::media_role::MediaRole;
use higgsfield_client::types::video_aspect_ratio::{KlingAspectRatio, SeedanceVideoAspectRatio};
use higgsfield_client::types::video_bitrate_mode::VideoBitrateMode;
use higgsfield_client::types::video_duration::{VideoDurationRange, VideoDurationSeconds};
use log::warn;

use crate::api::audio_list_ref::AudioListRef;
use crate::api::character_list_ref::CharacterListRef;
use crate::api::image_list_ref::ImageListRef;
use crate::api::image_ref::ImageRef;
use crate::api::router_aspect_ratio::RouterAspectRatio;
use crate::api::router_bitrate::RouterBitrate;
use crate::api::video_list_ref::VideoListRef;
use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::client_error::ClientError;
use crate::generate::generate_image::providers::higgsfield::common::{aspect_ratio_value, nearest_aspect_ratio};
use crate::generate::generate_video::generate_video_request_builder::GenerateVideoRequestBuilder;
use crate::generate::generate_video::providers::higgsfield::draft::{HiggsfieldVideoDraftState, HiggsfieldVideoPlan};
use crate::generate::generate_video::providers::higgsfield::request::HiggsfieldVideoRequestState;
use crate::generate::generate_video::video_generation_draft::VideoGenerationDraftRequest;
use crate::generate::generate_video::video_generation_draft_or_request::VideoGenerationDraftOrRequest;
use crate::generate::generate_video::video_generation_request::VideoGenerationRequest;

pub(crate) use crate::generate::generate_image::providers::higgsfield::common::{require_prompt, snap_resolution};

/// Clip length: in range passes through; out of range clamps (or errors out
/// under `ErrorOut`). `None` means `default`.
pub(crate) fn plan_duration(
  requested: Option<u16>,
  range: VideoDurationRange,
  default: u32,
  strategy: RequestMismatchMitigationStrategy,
) -> Result<VideoDurationSeconds, ArtcraftRouterError> {
  let Some(requested) = requested else {
    return Ok(VideoDurationSeconds::new(default));
  };
  let duration = VideoDurationSeconds::new(requested as u32);
  if range.contains(duration) {
    return Ok(duration);
  }
  if let RequestMismatchMitigationStrategy::ErrorOut = strategy {
    return Err(ArtcraftRouterError::Client(ClientError::ModelDoesNotSupportOption {
      field: "duration_seconds",
      value: format!("{requested} (Higgsfield offers {}–{}s for this model)", range.min_seconds, range.max_seconds),
    }));
  }
  let clamped = (requested as u32).clamp(range.min_seconds, range.max_seconds);
  warn!("Higgsfield offers {}–{}s for this model; clamping {}s to {}s", range.min_seconds, range.max_seconds, requested, clamped);
  Ok(VideoDurationSeconds::new(clamped))
}

/// The Seedance models render 1–4 clips per request; larger batches clamp
/// to 4 (or error out under `ErrorOut`).
pub(crate) fn plan_batch_size(
  video_batch_count: Option<u16>,
  strategy: RequestMismatchMitigationStrategy,
) -> Result<ImageBatchSize, ArtcraftRouterError> {
  let count = video_batch_count.unwrap_or(1);
  if count == 0 {
    return Err(ArtcraftRouterError::Client(ClientError::UserRequestedZeroGenerations));
  }
  if let Some(batch_size) = ImageBatchSize::from_u32(count as u32) {
    return Ok(batch_size);
  }
  match strategy {
    RequestMismatchMitigationStrategy::ErrorOut => Err(ArtcraftRouterError::Client(ClientError::ModelDoesNotSupportOption {
      field: "video_batch_count",
      value: format!("{count} (Higgsfield renders at most {} per request)", ImageBatchSize::MAX),
    })),
    _ => {
      warn!("Higgsfield renders at most {} videos per request; clamping {}", ImageBatchSize::MAX, count);
      Ok(ImageBatchSize::Four)
    }
  }
}

/// Models with no batch control render one clip; a bigger request is an
/// error under `ErrorOut` and otherwise renders one.
pub(crate) fn plan_single_video(
  video_batch_count: Option<u16>,
  strategy: RequestMismatchMitigationStrategy,
  model: &str,
) -> Result<(), ArtcraftRouterError> {
  match video_batch_count.unwrap_or(1) {
    0 => Err(ArtcraftRouterError::Client(ClientError::UserRequestedZeroGenerations)),
    1 => Ok(()),
    count => match strategy {
      RequestMismatchMitigationStrategy::ErrorOut => Err(ArtcraftRouterError::Client(ClientError::ModelDoesNotSupportOption {
        field: "video_batch_count",
        value: format!("{count} ({model} on Higgsfield renders one video per request)"),
      })),
      _ => {
        warn!("{model} on Higgsfield renders one video per request; ignoring batch of {count}");
        Ok(())
      }
    },
  }
}

/// The Seedance menu: 21:9, 16:9, 4:3, 1:1, 3:4, 9:16, and (for the models
/// that take it) Auto. Unset / auto ratios become Auto when `allow_auto`,
/// else 16:9; everything else snaps to the nearest ratio.
pub(crate) fn plan_seedance_aspect_ratio(requested: Option<RouterAspectRatio>, allow_auto: bool) -> SeedanceVideoAspectRatio {
  use SeedanceVideoAspectRatio as Ar;
  const CANDIDATES: &[(f64, Ar)] = &[
    (21.0 / 9.0, Ar::Landscape21x9),
    (16.0 / 9.0, Ar::Landscape16x9),
    (4.0 / 3.0, Ar::Landscape4x3),
    (1.0, Ar::Square1x1),
    (3.0 / 4.0, Ar::Portrait3x4),
    (9.0 / 16.0, Ar::Portrait9x16),
  ];
  match requested.and_then(aspect_ratio_value) {
    None if allow_auto => Ar::Auto,
    None => Ar::Landscape16x9,
    Some(value) => nearest_aspect_ratio(value, CANDIDATES),
  }
}

/// Kling's menu: 16:9, 9:16, 1:1. Unset / auto ratios are 16:9.
pub(crate) fn plan_kling_aspect_ratio(requested: Option<RouterAspectRatio>) -> KlingAspectRatio {
  use KlingAspectRatio as Ar;
  const CANDIDATES: &[(f64, Ar)] = &[
    (16.0 / 9.0, Ar::Landscape16x9),
    (9.0 / 16.0, Ar::Portrait9x16),
    (1.0, Ar::Square1x1),
  ];
  match requested.and_then(aspect_ratio_value) {
    None => Ar::Landscape16x9,
    Some(value) => nearest_aspect_ratio(value, CANDIDATES),
  }
}

/// Seedance bitrate: the web app defaults to high.
pub(crate) fn plan_bitrate(bitrate: Option<RouterBitrate>) -> VideoBitrateMode {
  match bitrate {
    None | Some(RouterBitrate::High) => VideoBitrateMode::High,
    Some(RouterBitrate::Normal) => VideoBitrateMode::Standard,
  }
}

/// A model has no menu for a field the caller set: say so, once.
pub(crate) fn warn_ignored<T: std::fmt::Debug>(model: &str, field: &str, value: Option<T>) {
  if let Some(value) = value {
    warn!("{model} on Higgsfield has no {field} control; ignoring {value:?}");
  }
}

/// The media a video request came with, before upload. Taken off the
/// builder as a unit so every model triages it the same way.
#[derive(Clone, Debug, Default)]
pub struct HiggsfieldVideoReferences {
  pub start_frame: Option<ImageRef>,
  pub end_frame: Option<ImageRef>,
  pub reference_images: Option<ImageListRef>,
  pub reference_videos: Option<VideoListRef>,
  pub reference_audio: Option<AudioListRef>,
}

impl HiggsfieldVideoReferences {
  /// Pull every reference off the builder. Character references have no
  /// Higgsfield equivalent and are dropped with a warning.
  pub fn take_from(builder: &mut GenerateVideoRequestBuilder, model: &str) -> Self {
    if let Some(CharacterListRef::CharacterTokens(tokens)) = builder.reference_character_tokens.take() {
      if !tokens.is_empty() {
        warn!("{model} on Higgsfield has no character references; dropping {}", tokens.len());
      }
    }
    Self {
      start_frame: builder.start_frame.take(),
      end_frame: builder.end_frame.take(),
      reference_images: builder.reference_images.take().filter(|list| list_len(list) > 0),
      reference_videos: builder.reference_videos.take().filter(|list| video_list_len(list) > 0),
      reference_audio: builder.reference_audio.take().filter(|list| audio_list_len(list) > 0),
    }
  }

  /// Drop every reference kind the model's endpoint doesn't accept, with a
  /// warning per kind, so a stray attachment doesn't fail the whole request.
  pub fn retain_roles(&mut self, allowed: &[MediaRole], model: &str) {
    if !allowed.contains(&MediaRole::StartImage) && self.start_frame.take().is_some() {
      warn!("{model} on Higgsfield takes no start frame; dropping it");
    }
    if !allowed.contains(&MediaRole::EndImage) && self.end_frame.take().is_some() {
      warn!("{model} on Higgsfield takes no end frame; dropping it");
    }
    if !allowed.contains(&MediaRole::Image) && self.reference_images.take().is_some() {
      warn!("{model} on Higgsfield takes no reference images; dropping them");
    }
    if !allowed.contains(&MediaRole::Video) && self.reference_videos.take().is_some() {
      warn!("{model} on Higgsfield takes no reference videos; dropping them");
    }
    if !allowed.contains(&MediaRole::Audio) && self.reference_audio.take().is_some() {
      warn!("{model} on Higgsfield takes no reference audio; dropping it");
    }
  }

  /// Reject over-limit lists before anything is uploaded.
  pub fn check_limits(&self, max_images: usize, max_videos: usize, max_audio: usize, model: &str) -> Result<(), ArtcraftRouterError> {
    check_limit(self.reference_image_count(), max_images, "reference_images", "reference images", model)?;
    check_limit(self.reference_video_count(), max_videos, "reference_videos", "reference videos", model)?;
    check_limit(self.reference_audio_count(), max_audio, "reference_audio", "reference audio files", model)
  }

  pub fn is_empty(&self) -> bool {
    self.start_frame.is_none()
        && self.end_frame.is_none()
        && self.reference_images.is_none()
        && self.reference_videos.is_none()
        && self.reference_audio.is_none()
  }

  pub fn reference_image_count(&self) -> usize {
    self.reference_images.as_ref().map(list_len).unwrap_or(0)
  }

  pub fn reference_video_count(&self) -> usize {
    self.reference_videos.as_ref().map(video_list_len).unwrap_or(0)
  }

  pub fn reference_audio_count(&self) -> usize {
    self.reference_audio.as_ref().map(audio_list_len).unwrap_or(0)
  }

  pub fn has_any_image(&self) -> bool {
    self.start_frame.is_some() || self.end_frame.is_some() || self.reference_image_count() > 0
  }
}

fn check_limit(count: usize, max: usize, field: &'static str, what: &str, model: &str) -> Result<(), ArtcraftRouterError> {
  if count > max {
    return Err(ArtcraftRouterError::Client(ClientError::ModelDoesNotSupportOption {
      field,
      value: format!("{count} {what} ({model} on Higgsfield takes at most {max})"),
    }));
  }
  Ok(())
}

fn list_len(list: &ImageListRef) -> usize {
  list.len()
}

fn video_list_len(list: &VideoListRef) -> usize {
  list.len()
}

fn audio_list_len(list: &AudioListRef) -> usize {
  list.len()
}

/// Wrap a plan: with media to upload it's a draft, otherwise it's ready to
/// send. `ip_check` is whether uploads should run Higgsfield's IP check (the
/// Seedance models refuse unchecked media).
pub(crate) fn finish(plan: HiggsfieldVideoPlan, references: HiggsfieldVideoReferences, ip_check: bool) -> VideoGenerationDraftOrRequest {
  if references.is_empty() {
    if let HiggsfieldVideoPlan::Request(request) = plan {
      return VideoGenerationDraftOrRequest::Request(
        VideoGenerationRequest::HiggsfieldVideo(HiggsfieldVideoRequestState { request }),
      );
    }
  }
  VideoGenerationDraftOrRequest::Draft(VideoGenerationDraftRequest::HiggsfieldVideo(HiggsfieldVideoDraftState {
    plan,
    unhandled_request_state: Some(references),
    ip_check,
  }))
}

#[cfg(test)]
mod tests {
  use super::*;

  mod durations_and_batches {
    use super::*;

    const RANGE: VideoDurationRange = VideoDurationRange::new(4, 15);

    #[test]
    fn in_range_passes_and_none_defaults() {
      assert_eq!(plan_duration(Some(10), RANGE, 5, RequestMismatchMitigationStrategy::ErrorOut).unwrap().seconds(), 10);
      assert_eq!(plan_duration(None, RANGE, 5, RequestMismatchMitigationStrategy::ErrorOut).unwrap().seconds(), 5);
    }

    #[test]
    fn out_of_range_clamps_or_errors() {
      assert_eq!(plan_duration(Some(30), RANGE, 5, RequestMismatchMitigationStrategy::PayMoreUpgrade).unwrap().seconds(), 15);
      assert_eq!(plan_duration(Some(1), RANGE, 5, RequestMismatchMitigationStrategy::PayLessDowngrade).unwrap().seconds(), 4);
      assert!(matches!(
        plan_duration(Some(30), RANGE, 5, RequestMismatchMitigationStrategy::ErrorOut),
        Err(ArtcraftRouterError::Client(ClientError::ModelDoesNotSupportOption { field: "duration_seconds", .. })),
      ));
    }

    #[test]
    fn batches() {
      assert_eq!(plan_batch_size(Some(2), RequestMismatchMitigationStrategy::ErrorOut).unwrap(), ImageBatchSize::Two);
      assert_eq!(plan_batch_size(Some(9), RequestMismatchMitigationStrategy::PayLessDowngrade).unwrap(), ImageBatchSize::Four);
      assert!(plan_batch_size(Some(9), RequestMismatchMitigationStrategy::ErrorOut).is_err());
      assert!(plan_batch_size(Some(0), RequestMismatchMitigationStrategy::ErrorOut).is_err());
      assert!(plan_single_video(Some(1), RequestMismatchMitigationStrategy::ErrorOut, "x").is_ok());
      assert!(plan_single_video(Some(3), RequestMismatchMitigationStrategy::PayMoreUpgrade, "x").is_ok());
      assert!(plan_single_video(Some(3), RequestMismatchMitigationStrategy::ErrorOut, "x").is_err());
    }
  }

  mod aspect_ratios_and_bitrate {
    use super::*;

    #[test]
    fn seedance() {
      use SeedanceVideoAspectRatio as Ar;
      assert_eq!(plan_seedance_aspect_ratio(None, true), Ar::Auto);
      assert_eq!(plan_seedance_aspect_ratio(None, false), Ar::Landscape16x9);
      assert_eq!(plan_seedance_aspect_ratio(Some(RouterAspectRatio::Auto), false), Ar::Landscape16x9);
      assert_eq!(plan_seedance_aspect_ratio(Some(RouterAspectRatio::Auto), true), Ar::Auto);
      assert_eq!(plan_seedance_aspect_ratio(Some(RouterAspectRatio::WideThreeByTwo), true), Ar::Landscape4x3);
      assert_eq!(plan_seedance_aspect_ratio(Some(RouterAspectRatio::TallTwoByThree), true), Ar::Portrait3x4);
      assert_eq!(plan_seedance_aspect_ratio(Some(RouterAspectRatio::TallNineByTwentyOne), true), Ar::Portrait9x16);
      assert_eq!(plan_seedance_aspect_ratio(Some(RouterAspectRatio::WideTwentyOneByNine), true), Ar::Landscape21x9);
      assert_eq!(plan_seedance_aspect_ratio(Some(RouterAspectRatio::Wide), true), Ar::Landscape16x9);
    }

    #[test]
    fn kling() {
      use KlingAspectRatio as Ar;
      assert_eq!(plan_kling_aspect_ratio(None), Ar::Landscape16x9);
      assert_eq!(plan_kling_aspect_ratio(Some(RouterAspectRatio::Auto)), Ar::Landscape16x9);
      assert_eq!(plan_kling_aspect_ratio(Some(RouterAspectRatio::WideThreeByTwo)), Ar::Landscape16x9);
      assert_eq!(plan_kling_aspect_ratio(Some(RouterAspectRatio::WideTwentyOneByNine)), Ar::Landscape16x9);
      assert_eq!(plan_kling_aspect_ratio(Some(RouterAspectRatio::TallTwoByThree)), Ar::Portrait9x16);
      // Nearest by ratio value: 4:3 (1.33) is closer to 1:1 than to 16:9,
      // while 3:4 (0.75) is closer to 9:16 (0.56) than to 1:1.
      assert_eq!(plan_kling_aspect_ratio(Some(RouterAspectRatio::WideFourByThree)), Ar::Square1x1);
      assert_eq!(plan_kling_aspect_ratio(Some(RouterAspectRatio::TallThreeByFour)), Ar::Portrait9x16);
      assert_eq!(plan_kling_aspect_ratio(Some(RouterAspectRatio::WideFiveByFour)), Ar::Square1x1);
      assert_eq!(plan_kling_aspect_ratio(Some(RouterAspectRatio::Square)), Ar::Square1x1);
    }

    #[test]
    fn bitrate() {
      assert_eq!(plan_bitrate(None), VideoBitrateMode::High);
      assert_eq!(plan_bitrate(Some(RouterBitrate::High)), VideoBitrateMode::High);
      assert_eq!(plan_bitrate(Some(RouterBitrate::Normal)), VideoBitrateMode::Standard);
    }
  }

  mod references {
    use super::*;
    use sqlite_identifiers::ids::media_file_token::MediaFileToken;

    fn full_builder() -> GenerateVideoRequestBuilder {
      GenerateVideoRequestBuilder {
        start_frame: Some(ImageRef::Url("s".into())),
        end_frame: Some(ImageRef::Url("e".into())),
        reference_images: Some(ImageListRef::Urls(vec!["a".into(), "b".into()])),
        reference_videos: Some(VideoListRef::MediaFileTokens(vec![MediaFileToken::new_from_str("m_v")])),
        reference_audio: Some(AudioListRef::Urls(vec!["x".into()])),
        reference_character_tokens: Some(CharacterListRef::CharacterTokens(vec![])),
        ..Default::default()
      }
    }

    #[test]
    fn takes_everything_and_drops_empty_lists() {
      let mut builder = full_builder();
      builder.reference_audio = Some(AudioListRef::Urls(vec![]));
      let references = HiggsfieldVideoReferences::take_from(&mut builder, "x");
      assert!(references.start_frame.is_some() && references.end_frame.is_some());
      assert_eq!(references.reference_image_count(), 2);
      assert_eq!(references.reference_video_count(), 1);
      assert_eq!(references.reference_audio_count(), 0);
      assert!(references.reference_audio.is_none());
      assert!(builder.start_frame.is_none() && builder.reference_images.is_none());
      assert!(!references.is_empty());
      assert!(HiggsfieldVideoReferences::default().is_empty());
    }

    #[test]
    fn retain_roles_drops_what_the_model_cannot_take() {
      let mut references = HiggsfieldVideoReferences::take_from(&mut full_builder(), "x");
      references.retain_roles(&[MediaRole::StartImage, MediaRole::Image], "x");
      assert!(references.start_frame.is_some());
      assert!(references.end_frame.is_none());
      assert_eq!(references.reference_image_count(), 2);
      assert!(references.reference_videos.is_none());
      assert!(references.reference_audio.is_none());
    }

    #[test]
    fn limits() {
      let references = HiggsfieldVideoReferences::take_from(&mut full_builder(), "x");
      assert!(references.check_limits(2, 1, 1, "x").is_ok());
      assert!(matches!(
        references.check_limits(1, 1, 1, "x"),
        Err(ArtcraftRouterError::Client(ClientError::ModelDoesNotSupportOption { field: "reference_images", .. })),
      ));
      assert!(matches!(
        references.check_limits(2, 0, 1, "x"),
        Err(ArtcraftRouterError::Client(ClientError::ModelDoesNotSupportOption { field: "reference_videos", .. })),
      ));
    }
  }
}
