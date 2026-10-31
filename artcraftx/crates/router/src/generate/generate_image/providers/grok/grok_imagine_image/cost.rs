use crate::generate::generate_image::image_generation_cost_estimate::ImageGenerationCostEstimate;
use crate::generate::generate_image::providers::grok::grok_imagine_image::request::GrokImagineImageRequestState;

/// Cost state for first-party (cookie-session) Grok Imagine image generation.
///
/// TODO(pricing): this runs on the USER'S OWN Grok account, so it does not
/// consume Artcraft credits. Placeholder reporting it as free/unlimited, like
/// the first-party Midjourney path; revisit the real pricing decision before
/// shipping.
pub struct GrokImagineImageCostState;

impl GrokImagineImageCostState {
  pub fn from_request(_request: &GrokImagineImageRequestState) -> Self {
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
