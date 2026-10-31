//! Seedance 2.5 Edit on Higgsfield: video-to-video. The first reference
//! video is the clip to edit; image and audio references guide the edit.
//! 480p / 720p / 1080p, 1–4 clips. The output follows the source clip, so
//! aspect ratio and duration have no effect and are ignored.

use higgsfield_client::endpoints::generate::video::seedance_2p5::Seedance2p5Resolution;
use higgsfield_client::endpoints::generate::video::seedance_2p5_edit::Seedance2p5EditRequest;
use higgsfield_client::types::media_role::MediaRole;

use crate::api::router_resolution::RouterResolution;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_video::generate_video_request_builder::GenerateVideoRequestBuilder;
use crate::generate::generate_video::providers::higgsfield::common::{
  finish, plan_batch_size, plan_bitrate, require_prompt, snap_resolution, warn_ignored, HiggsfieldVideoReferences,
};
use crate::generate::generate_video::providers::higgsfield::draft::{HiggsfieldVideoPlan, Seedance2p5EditPlan};
use crate::generate::generate_video::video_generation_draft_or_request::VideoGenerationDraftOrRequest;

const MODEL: &str = "Seedance 2.5 Edit";
pub const MAX_REFERENCE_IMAGES: usize = 9;
/// The source clip plus nothing else.
pub const MAX_REFERENCE_VIDEOS: usize = 1;
pub const MAX_REFERENCE_AUDIO: usize = 3;

const RESOLUTION_TIERS: &[(RouterResolution, Seedance2p5Resolution)] = &[
  (RouterResolution::FourEightyP, Seedance2p5Resolution::P480),
  (RouterResolution::SevenTwentyP, Seedance2p5Resolution::P720),
  (RouterResolution::TenEightyP, Seedance2p5Resolution::P1080),
];

/// The roles the edit takes on top of its source clip.
const ACCEPTED_ROLES: &[MediaRole] = &[MediaRole::Video, MediaRole::Image, MediaRole::Audio];

pub fn build_higgsfield_seedance_2p5_edit(mut builder: GenerateVideoRequestBuilder) -> Result<VideoGenerationDraftOrRequest, ArtcraftRouterError> {
  let strategy = builder.request_mismatch_mitigation_strategy;
  let prompt = require_prompt(builder.prompt.take())?;
  warn_ignored(MODEL, "negative prompt", builder.negative_prompt.take());
  warn_ignored(MODEL, "aspect ratio", builder.aspect_ratio.take());
  warn_ignored(MODEL, "duration", builder.duration_seconds.take());

  let mut references = HiggsfieldVideoReferences::take_from(&mut builder, MODEL);
  references.retain_roles(ACCEPTED_ROLES, MODEL);
  if references.reference_video_count() == 0 {
    return Err(ArtcraftRouterError::InvalidInput("Seedance 2.5 Edit needs the video to edit as a reference video".to_string()));
  }
  references.check_limits(MAX_REFERENCE_IMAGES, MAX_REFERENCE_VIDEOS, MAX_REFERENCE_AUDIO, MODEL)?;
  debug_assert!(Seedance2p5EditRequest::MEDIA_ROLES.iter().all(|role| ACCEPTED_ROLES.contains(role)));

  let plan = Seedance2p5EditPlan {
    prompt,
    resolution: snap_resolution(builder.resolution.take(), RESOLUTION_TIERS, Seedance2p5Resolution::P720, strategy)?,
    batch_size: plan_batch_size(builder.video_batch_count.take(), strategy)?,
    generate_audio: builder.generate_audio.take().unwrap_or(true),
    bitrate_mode: plan_bitrate(builder.bitrate.take()),
  };

  Ok(finish(HiggsfieldVideoPlan::Seedance2p5Edit(plan), references, true))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_video_model::RouterVideoModel;
  use crate::generate::generate_video::providers::higgsfield::common_test_helpers::{base_builder, unwrap_draft, with_reference_videos, with_start_frame};

  fn builder() -> GenerateVideoRequestBuilder {
    with_reference_videos(base_builder(RouterVideoModel::Seedance2p5Edit), 1)
  }

  #[test]
  fn needs_a_source_video() {
    let err = build_higgsfield_seedance_2p5_edit(base_builder(RouterVideoModel::Seedance2p5Edit)).unwrap_err();
    assert!(matches!(err, ArtcraftRouterError::InvalidInput(_)));
    assert!(build_higgsfield_seedance_2p5_edit(with_reference_videos(base_builder(RouterVideoModel::Seedance2p5Edit), 2)).is_err());
  }

  #[test]
  fn always_drafts_with_an_edit_plan() {
    let mut b = builder();
    b.resolution = Some(RouterResolution::FourK);
    b.video_batch_count = Some(2);
    b.generate_audio = Some(false);
    let draft = unwrap_draft(build_higgsfield_seedance_2p5_edit(b).expect("build"));
    assert!(draft.ip_check);
    match draft.plan {
      HiggsfieldVideoPlan::Seedance2p5Edit(plan) => {
        assert_eq!(plan.resolution, Seedance2p5Resolution::P1080);
        assert_eq!(plan.batch_size.as_u32(), 2);
        assert!(!plan.generate_audio);
      }
      other => panic!("expected an edit plan, got {other:?}"),
    }
    assert_eq!(draft.unhandled_request_state.unwrap().reference_video_count(), 1);
  }

  #[test]
  fn keyframes_are_dropped_not_rejected() {
    let draft = unwrap_draft(build_higgsfield_seedance_2p5_edit(with_start_frame(builder())).expect("build"));
    assert!(draft.unhandled_request_state.unwrap().start_frame.is_none());
  }
}
