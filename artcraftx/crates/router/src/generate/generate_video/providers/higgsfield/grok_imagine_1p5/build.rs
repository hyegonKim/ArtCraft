//! Grok Imagine 1.5 on Higgsfield: text- or image-to-video at 480p / 720p /
//! 1080p, 1–15s, one clip, a start frame and image references. The output
//! follows the start frame's aspect ("auto"); a requested ratio is ignored.

use higgsfield_client::endpoints::generate::video::grok_imagine_1p5::{GrokImagine1p5Request, GrokImagine1p5Resolution};

use crate::api::router_resolution::RouterResolution;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_video::generate_video_request_builder::GenerateVideoRequestBuilder;
use crate::generate::generate_video::providers::higgsfield::common::{
  finish, plan_duration, plan_single_video, require_prompt, snap_resolution, warn_ignored, HiggsfieldVideoReferences,
};
use crate::generate::generate_video::providers::higgsfield::draft::HiggsfieldVideoPlan;
use crate::generate::generate_video::providers::higgsfield::video_request::HiggsfieldVideoRequest;
use crate::generate::generate_video::video_generation_draft_or_request::VideoGenerationDraftOrRequest;

const MODEL: &str = "Grok Imagine 1.5";
pub const MAX_REFERENCE_IMAGES: usize = 4;
const DEFAULT_DURATION_SECONDS: u32 = 6;

const RESOLUTION_TIERS: &[(RouterResolution, GrokImagine1p5Resolution)] = &[
  (RouterResolution::FourEightyP, GrokImagine1p5Resolution::P480),
  (RouterResolution::SevenTwentyP, GrokImagine1p5Resolution::P720),
  (RouterResolution::TenEightyP, GrokImagine1p5Resolution::P1080),
];

pub fn build_higgsfield_grok_imagine_1p5(mut builder: GenerateVideoRequestBuilder) -> Result<VideoGenerationDraftOrRequest, ArtcraftRouterError> {
  let strategy = builder.request_mismatch_mitigation_strategy;
  let prompt = require_prompt(builder.prompt.take())?;
  warn_ignored(MODEL, "negative prompt", builder.negative_prompt.take());
  warn_ignored(MODEL, "aspect ratio", builder.aspect_ratio.take());
  warn_ignored(MODEL, "bitrate", builder.bitrate.take());
  warn_ignored(MODEL, "audio toggle", builder.generate_audio.take());
  plan_single_video(builder.video_batch_count.take(), strategy, MODEL)?;

  let mut references = HiggsfieldVideoReferences::take_from(&mut builder, MODEL);
  references.retain_roles(GrokImagine1p5Request::MEDIA_ROLES, MODEL);
  references.check_limits(MAX_REFERENCE_IMAGES, 0, 0, MODEL)?;

  let resolution = snap_resolution(builder.resolution.take(), RESOLUTION_TIERS, GrokImagine1p5Resolution::P720, strategy)?;
  let duration = plan_duration(builder.duration_seconds.take(), GrokImagine1p5Request::DURATION, DEFAULT_DURATION_SECONDS, strategy)?;
  let request = GrokImagine1p5Request::text_to_video(prompt, resolution, duration);

  Ok(finish(HiggsfieldVideoPlan::Request(HiggsfieldVideoRequest::GrokImagine1p5(request)), references, false))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::image_ref::ImageRef;
  use crate::api::router_video_model::RouterVideoModel;
  use crate::generate::generate_video::providers::higgsfield::common_test_helpers::{base_builder, unwrap_draft, unwrap_request, with_start_frame};

  fn built(builder: GenerateVideoRequestBuilder) -> GrokImagine1p5Request {
    match unwrap_request(build_higgsfield_grok_imagine_1p5(builder).expect("build")) {
      HiggsfieldVideoRequest::GrokImagine1p5(request) => request,
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn text_to_video_works_without_a_frame() {
    let request = built(base_builder(RouterVideoModel::GrokImagineVideo1p5));
    assert_eq!(request.resolution, GrokImagine1p5Resolution::P720);
    assert_eq!(request.duration.seconds(), 6);
  }

  #[test]
  fn four_k_snaps_to_1080p_and_durations_clamp() {
    let mut b = base_builder(RouterVideoModel::GrokImagineVideo1p5);
    b.resolution = Some(RouterResolution::FourK);
    b.duration_seconds = Some(20);
    let request = built(b);
    assert_eq!(request.resolution, GrokImagine1p5Resolution::P1080);
    assert_eq!(request.duration.seconds(), 15);
  }

  #[test]
  fn end_frames_are_dropped_but_start_frames_kept() {
    let mut b = with_start_frame(base_builder(RouterVideoModel::GrokImagineVideo1p5));
    b.end_frame = Some(ImageRef::Url("https://cdn.example.com/end.png".to_string()));
    let draft = unwrap_draft(build_higgsfield_grok_imagine_1p5(b).expect("build"));
    let references = draft.unhandled_request_state.unwrap();
    assert!(references.start_frame.is_some());
    assert!(references.end_frame.is_none());
  }
}
