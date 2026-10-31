use crate::generate::generate_video::providers::higgsfield::draft::HiggsfieldVideoDraftState;
use crate::generate::generate_video::providers::higgsfield::request::HiggsfieldVideoRequestState;
use crate::generate::generate_video::video_generation_cost_estimate::VideoGenerationCostEstimate;

/// Cost state for first-party (cookie-session) Higgsfield video generation.
///
/// This runs on the USER'S OWN Higgsfield account (its credits or unlimited
/// plan), so it consumes no ArtCraft credits. Reported as free / unlimited
/// like the other first-party providers; Higgsfield's own credit pricing
/// isn't modelled here.
pub struct HiggsfieldVideoCostState;

impl HiggsfieldVideoCostState {
  pub fn from_request(_request: &HiggsfieldVideoRequestState) -> Self {
    Self
  }

  pub fn from_draft(_draft: &HiggsfieldVideoDraftState) -> Self {
    Self
  }

  pub fn estimate_cost(&self) -> VideoGenerationCostEstimate {
    VideoGenerationCostEstimate {
      cost_in_credits: Some(0),
      cost_in_usd_cents: None,
      is_free: true,
      is_unlimited: true,
      is_rate_limited: false,
      has_watermark: false,
      failures_are_refunded: None,
    }
  }
}
