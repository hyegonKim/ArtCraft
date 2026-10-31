use crate::generate::generate_image::image_generation_cost_estimate::ImageGenerationCostEstimate;
use crate::generate::generate_image::providers::midjourney::midjourney_8::request::MidjourneyMidjourney8RequestState;

/// Cost state for first-party (cookie-session) Midjourney v8.
///
/// TODO(pricing): first-party Midjourney runs on the USER'S OWN Midjourney
/// subscription, so it does not consume Artcraft credits. This is a placeholder
/// that reports the generation as free/unlimited. The real product/pricing
/// decision (charge an Artcraft markup? keep it free? rate-limit?) is still open
/// — revisit before shipping. Any supplier-cost/margin figures belong in the
/// external margins repo, not here.
pub struct MidjourneyMidjourney8CostState;

impl MidjourneyMidjourney8CostState {
  pub fn from_request(_request: &MidjourneyMidjourney8RequestState) -> Self {
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
