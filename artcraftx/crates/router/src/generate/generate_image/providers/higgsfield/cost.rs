use crate::generate::generate_image::image_generation_cost_estimate::ImageGenerationCostEstimate;
use crate::generate::generate_image::providers::higgsfield::draft::HiggsfieldImageDraftState;
use crate::generate::generate_image::providers::higgsfield::request::HiggsfieldImageRequestState;

/// Cost state for first-party (cookie-session) Higgsfield image generation.
///
/// This runs on the USER'S OWN Higgsfield account (its credits or unlimited
/// plan), so it consumes no ArtCraft credits. Reported as free / unlimited
/// like the other first-party providers; Higgsfield's own credit pricing
/// isn't modelled here.
pub struct HiggsfieldImageCostState;

impl HiggsfieldImageCostState {
  pub fn from_request(_request: &HiggsfieldImageRequestState) -> Self {
    Self
  }

  pub fn from_draft(_draft: &HiggsfieldImageDraftState) -> Self {
    Self
  }

  pub fn estimate_cost(&self) -> ImageGenerationCostEstimate {
    ImageGenerationCostEstimate {
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
