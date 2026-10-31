use crate::generate::generate_splat::plan::artcraft::plan_generate_splat_artcraft_marble_0p1_mini::PlanArtcraftMarble0p1Mini;
use crate::generate::generate_splat::splat_generation_cost_estimate::SplatGenerationCostEstimate;

/// Marble 0.1-mini costs 230 World Labs credits for text or a single image
/// ($1 per 1,250 credits), so it is flat priced.
const COST_IN_USD_CENTS: u64 = 18;

pub(crate) fn estimate_splat_cost_artcraft_marble_0p1_mini(
  _plan: &PlanArtcraftMarble0p1Mini,
) -> SplatGenerationCostEstimate {
  let cost_in_usd_cents = COST_IN_USD_CENTS;

  SplatGenerationCostEstimate {
    cost_in_credits: Some(cost_in_usd_cents),
    cost_in_usd_cents: Some(cost_in_usd_cents),
    is_free: false,
    is_unlimited: false,
    is_rate_limited: false,
    has_watermark: false,
    failures_are_refunded: None,
  }
}

#[cfg(test)]
mod tests {
  use crate::api::router_splat_model::RouterSplatModel;
  use crate::api::router_provider::RouterProvider;
  use crate::generate::generate_splat::generate_splat_request::GenerateSplatRequest;

  use sqlite_identifiers::ids::media_file_token::MediaFileToken;

  fn estimate_usd_cents(prompt: Option<&str>, image_tokens: Option<Vec<MediaFileToken>>) -> u64 {
    let request = GenerateSplatRequest {
      model: RouterSplatModel::Marble0p1Mini,
      provider: RouterProvider::Artcraft,
      prompt: prompt.map(|s| s.to_string()),
      reference_images: image_tokens.map(crate::api::image_list_ref::ImageListRef::MediaFileTokens),
      idempotency_token: None,
    };
    request.build()
      .expect("build should succeed")
      .estimate_costs()
      .cost_in_usd_cents
      .expect("cost_in_usd_cents should be present")
  }

  #[test]
  fn test_estimate_cost_text_only() {
    let cost = estimate_usd_cents(Some("a room"), None);
    assert_eq!(cost, 18); // 230 credits → 18 cents
  }

  #[test]
  fn test_estimate_cost_no_image_no_prompt() {
    let cost = estimate_usd_cents(None, None);
    assert_eq!(cost, 18);
  }

  #[test]
  fn test_estimate_cost_image() {
    let tokens = vec![MediaFileToken("test_token".to_string())];
    let cost = estimate_usd_cents(None, Some(tokens.clone()));
    assert_eq!(cost, 18); // 230 credits → 18 cents (same as text for mini)
  }

  #[test]
  fn test_estimate_cost_image_with_prompt() {
    let tokens = vec![MediaFileToken("test_token".to_string())];
    let cost = estimate_usd_cents(Some("a cozy room"), Some(tokens.clone()));
    assert_eq!(cost, 18); // 230 credits → 18 cents
  }
}
