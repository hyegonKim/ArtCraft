//! Planning helpers shared by every Higgsfield image model: prompt and batch
//! validation, and "nearest supported option" snapping for resolutions and
//! aspect ratios.

use higgsfield_client::endpoints::generate::image::gpt_image_2::GptImage2AspectRatio;
use higgsfield_client::types::image_batch_size::ImageBatchSize;
use higgsfield_client::types::nano_banana_aspect_ratio::NanoBananaAspectRatio;
use higgsfield_client::types::seedream_aspect_ratio::SeedreamAspectRatio;
use log::warn;

use crate::api::image_list_ref::ImageListRef;
use crate::api::router_aspect_ratio::RouterAspectRatio;
use crate::api::router_resolution::RouterResolution;
use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::client_error::ClientError;
use crate::generate::generate_image::image_generation_draft::ImageGenerationDraftRequest;
use crate::generate::generate_image::image_generation_draft_or_request::ImageGenerationDraftOrRequest;
use crate::generate::generate_image::image_generation_request::ImageGenerationRequest;
use crate::generate::generate_image::providers::higgsfield::draft::{HiggsfieldImageDraftState, HiggsfieldImageRemainingItems};
use crate::generate::generate_image::providers::higgsfield::image_request::HiggsfieldImageRequest;
use crate::generate::generate_image::providers::higgsfield::request::HiggsfieldImageRequestState;

/// Higgsfield needs a text prompt for every image model.
pub(crate) fn require_prompt(prompt: Option<String>) -> Result<String, ArtcraftRouterError> {
  let prompt = prompt.unwrap_or_default().trim().to_string();
  if prompt.is_empty() {
    return Err(ArtcraftRouterError::InvalidInput("Higgsfield needs a text prompt".to_string()));
  }
  Ok(prompt)
}

pub(crate) fn image_input_count(image_inputs: Option<&ImageListRef>) -> usize {
  image_inputs.map_or(0, ImageListRef::len)
}

/// Reject over-limit reference lists before anything is uploaded.
pub(crate) fn check_reference_limit(count: usize, max: usize, model: &str) -> Result<(), ArtcraftRouterError> {
  if count > max {
    return Err(ArtcraftRouterError::Client(ClientError::ModelDoesNotSupportOption {
      field: "image_inputs",
      value: format!("{count} reference images ({model} on Higgsfield takes at most {max})"),
    }));
  }
  Ok(())
}

/// Every Higgsfield image model renders 1–4 images per request. Larger
/// batches clamp to 4 (or error out under `ErrorOut`).
pub(crate) fn plan_batch_size(
  image_batch_count: Option<u16>,
  strategy: RequestMismatchMitigationStrategy,
) -> Result<ImageBatchSize, ArtcraftRouterError> {
  let count = image_batch_count.unwrap_or(1);
  if count == 0 {
    return Err(ArtcraftRouterError::Client(ClientError::UserRequestedZeroGenerations));
  }
  if let Some(batch_size) = ImageBatchSize::from_u32(count as u32) {
    return Ok(batch_size);
  }
  match strategy {
    RequestMismatchMitigationStrategy::ErrorOut => Err(ArtcraftRouterError::Client(ClientError::ModelDoesNotSupportOption {
      field: "image_batch_count",
      value: format!("{count} (Higgsfield renders at most {} per request)", ImageBatchSize::MAX),
    })),
    _ => {
      warn!("Higgsfield renders at most {} images per request; clamping {}", ImageBatchSize::MAX, count);
      Ok(ImageBatchSize::Four)
    }
  }
}

/// A total order over resolutions so tiers can be compared across the
/// image (`*K`) and video (`*p`) vocabularies.
pub(crate) fn resolution_rank(resolution: RouterResolution) -> u8 {
  match resolution {
    RouterResolution::HalfK => 0,
    RouterResolution::FourEightyP => 1,
    RouterResolution::SevenTwentyP => 2,
    RouterResolution::OneK => 3,
    RouterResolution::TenEightyP => 4,
    RouterResolution::TwoK => 5,
    RouterResolution::ThreeK => 6,
    RouterResolution::FourK => 7,
  }
}

/// Pick the tier for a requested resolution from a model's supported
/// `tiers` (ascending). An exact match wins; otherwise `PayMoreUpgrade`
/// takes the next tier up (or the top one), `PayLessDowngrade` the next tier
/// down (or the bottom one), and `ErrorOut` refuses. `None` picks `default`.
pub(crate) fn snap_resolution<T: Copy>(
  requested: Option<RouterResolution>,
  tiers: &[(RouterResolution, T)],
  default: T,
  strategy: RequestMismatchMitigationStrategy,
) -> Result<T, ArtcraftRouterError> {
  debug_assert!(tiers.windows(2).all(|pair| resolution_rank(pair[0].0) < resolution_rank(pair[1].0)), "tiers must ascend");
  let Some(requested) = requested else {
    return Ok(default);
  };
  let rank = resolution_rank(requested);
  if let Some((_, tier)) = tiers.iter().find(|(supported, _)| resolution_rank(*supported) == rank) {
    return Ok(*tier);
  }
  let snapped = match strategy {
    RequestMismatchMitigationStrategy::ErrorOut => {
      return Err(ArtcraftRouterError::Client(ClientError::ModelDoesNotSupportOption {
        field: "resolution",
        value: format!("{requested:?}"),
      }));
    }
    RequestMismatchMitigationStrategy::PayMoreUpgrade => tiers.iter()
        .find(|(supported, _)| resolution_rank(*supported) > rank)
        .or_else(|| tiers.last()),
    RequestMismatchMitigationStrategy::PayLessDowngrade => tiers.iter()
        .rev()
        .find(|(supported, _)| resolution_rank(*supported) < rank)
        .or_else(|| tiers.first()),
  };
  let (snapped_resolution, tier) = snapped.expect("a model offers at least one resolution tier");
  warn!("Higgsfield does not offer {:?} for this model; using {:?} ({:?})", requested, snapped_resolution, strategy);
  Ok(*tier)
}

/// A ratio's numeric value (width / height). `None` for the auto values;
/// the loose `Wide` / `Tall` read as 16:9 / 9:16.
pub(crate) fn aspect_ratio_value(aspect_ratio: RouterAspectRatio) -> Option<f64> {
  let value = match aspect_ratio {
    RouterAspectRatio::Auto
    | RouterAspectRatio::Auto2k
    | RouterAspectRatio::Auto3k
    | RouterAspectRatio::Auto4k => return None,
    RouterAspectRatio::Square | RouterAspectRatio::SquareHd => 1.0,
    RouterAspectRatio::WideThreeByTwo => 3.0 / 2.0,
    RouterAspectRatio::WideFourByThree => 4.0 / 3.0,
    RouterAspectRatio::WideFiveByFour => 5.0 / 4.0,
    RouterAspectRatio::WideSixteenByNine | RouterAspectRatio::Wide => 16.0 / 9.0,
    RouterAspectRatio::WideTwentyOneByNine => 21.0 / 9.0,
    RouterAspectRatio::TallTwoByThree => 2.0 / 3.0,
    RouterAspectRatio::TallThreeByFour => 3.0 / 4.0,
    RouterAspectRatio::TallFourByFive => 4.0 / 5.0,
    RouterAspectRatio::TallNineBySixteen | RouterAspectRatio::Tall => 9.0 / 16.0,
    RouterAspectRatio::TallNineByTwentyOne => 9.0 / 21.0,
  };
  Some(value)
}

/// The candidate whose numeric ratio is closest to `value` (ties go to the
/// earlier candidate).
pub(crate) fn nearest_aspect_ratio<T: Copy>(value: f64, candidates: &[(f64, T)]) -> T {
  let mut best = candidates[0];
  for candidate in candidates {
    if (candidate.0 - value).abs() < (best.0 - value).abs() - f64::EPSILON {
      best = *candidate;
    }
  }
  best.1
}

/// The Nano Banana aspect menu (Pro / 2 / 2 Lite): every named ratio plus
/// Auto. With reference images Auto follows the reference, so an unset ratio
/// means Auto when editing and 16:9 for text-to-image.
pub(crate) fn plan_nano_banana_aspect_ratio(requested: Option<RouterAspectRatio>, editing: bool) -> NanoBananaAspectRatio {
  use NanoBananaAspectRatio as Ar;
  const CANDIDATES: &[(f64, Ar)] = &[
    (1.0, Ar::Square1x1),
    (3.0 / 4.0, Ar::Portrait3x4),
    (4.0 / 3.0, Ar::Landscape4x3),
    (2.0 / 3.0, Ar::Portrait2x3),
    (3.0 / 2.0, Ar::Landscape3x2),
    (9.0 / 16.0, Ar::Portrait9x16),
    (16.0 / 9.0, Ar::Landscape16x9),
    (5.0 / 4.0, Ar::Landscape5x4),
    (4.0 / 5.0, Ar::Portrait4x5),
    (21.0 / 9.0, Ar::Landscape21x9),
  ];
  match requested {
    None if editing => Ar::Auto,
    None => Ar::Landscape16x9,
    Some(ratio) => match aspect_ratio_value(ratio) {
      None => Ar::Auto,
      Some(value) => nearest_aspect_ratio(value, CANDIDATES),
    },
  }
}

/// GPT Image 2's aspect menu: like Nano Banana's but without 5:4 / 4:5.
pub(crate) fn plan_gpt_image_2_aspect_ratio(requested: Option<RouterAspectRatio>, editing: bool) -> GptImage2AspectRatio {
  use GptImage2AspectRatio as Ar;
  const CANDIDATES: &[(f64, Ar)] = &[
    (1.0, Ar::Square1x1),
    (3.0 / 2.0, Ar::Landscape3x2),
    (2.0 / 3.0, Ar::Portrait2x3),
    (16.0 / 9.0, Ar::Landscape16x9),
    (9.0 / 16.0, Ar::Portrait9x16),
    (4.0 / 3.0, Ar::Landscape4x3),
    (3.0 / 4.0, Ar::Portrait3x4),
    (21.0 / 9.0, Ar::Landscape21x9),
  ];
  match requested {
    None if editing => Ar::Auto,
    None => Ar::Landscape16x9,
    Some(ratio) => match aspect_ratio_value(ratio) {
      None => Ar::Auto,
      Some(value) => nearest_aspect_ratio(value, CANDIDATES),
    },
  }
}

/// The Seedream menu has no Auto: unset and auto ratios fall back to
/// `default`, everything else snaps to the nearest of its eight ratios.
pub(crate) fn plan_seedream_aspect_ratio(requested: Option<RouterAspectRatio>, default: SeedreamAspectRatio) -> SeedreamAspectRatio {
  use SeedreamAspectRatio as Ar;
  const CANDIDATES: &[(f64, Ar)] = &[
    (1.0, Ar::Square1x1),
    (4.0 / 3.0, Ar::Landscape4x3),
    (3.0 / 4.0, Ar::Portrait3x4),
    (16.0 / 9.0, Ar::Landscape16x9),
    (21.0 / 9.0, Ar::Landscape21x9),
    (9.0 / 16.0, Ar::Portrait9x16),
    (2.0 / 3.0, Ar::Portrait2x3),
    (3.0 / 2.0, Ar::Landscape3x2),
  ];
  match requested.and_then(aspect_ratio_value) {
    None => default,
    Some(value) => nearest_aspect_ratio(value, CANDIDATES),
  }
}

/// ArtCraft's Seedream models bake a resolution into their "auto" ratios
/// (`auto_2k`, ...). Higgsfield has a real resolution menu, so when no
/// resolution was given the baked-in one becomes the request.
pub(crate) fn seedream_auto_resolution_hint(requested: Option<RouterAspectRatio>) -> Option<RouterResolution> {
  match requested? {
    RouterAspectRatio::Auto2k => Some(RouterResolution::TwoK),
    RouterAspectRatio::Auto3k => Some(RouterResolution::ThreeK),
    RouterAspectRatio::Auto4k => Some(RouterResolution::FourK),
    _ => None,
  }
}

/// Wrap a planned request: with reference images it's a draft (they still
/// have to be uploaded), otherwise it's ready to send.
pub(crate) fn finish(request: HiggsfieldImageRequest, image_inputs: Option<ImageListRef>) -> ImageGenerationDraftOrRequest {
  if image_input_count(image_inputs.as_ref()) == 0 {
    return ImageGenerationDraftOrRequest::Request(
      ImageGenerationRequest::HiggsfieldImage(HiggsfieldImageRequestState { request }),
    );
  }
  ImageGenerationDraftOrRequest::Draft(ImageGenerationDraftRequest::HiggsfieldImage(HiggsfieldImageDraftState {
    request,
    unhandled_request_state: Some(HiggsfieldImageRemainingItems { reference_images: image_inputs }),
  }))
}

#[cfg(test)]
mod tests {
  use super::*;
  use higgsfield_client::endpoints::generate::image::nano_banana_pro::NanoBananaProResolution;

  const NANO_BANANA_PRO_TIERS: &[(RouterResolution, NanoBananaProResolution)] = &[
    (RouterResolution::OneK, NanoBananaProResolution::OneK),
    (RouterResolution::TwoK, NanoBananaProResolution::TwoK),
    (RouterResolution::FourK, NanoBananaProResolution::FourK),
  ];

  mod prompts_and_batches {
    use super::*;

    #[test]
    fn prompt_is_trimmed_and_required() {
      assert_eq!(require_prompt(Some("  a cat  ".to_string())).unwrap(), "a cat");
      assert!(matches!(require_prompt(Some("   ".to_string())), Err(ArtcraftRouterError::InvalidInput(_))));
      assert!(matches!(require_prompt(None), Err(ArtcraftRouterError::InvalidInput(_))));
    }

    #[test]
    fn batch_sizes_clamp_to_four() {
      assert_eq!(plan_batch_size(None, RequestMismatchMitigationStrategy::ErrorOut).unwrap(), ImageBatchSize::One);
      assert_eq!(plan_batch_size(Some(3), RequestMismatchMitigationStrategy::ErrorOut).unwrap(), ImageBatchSize::Three);
      assert_eq!(plan_batch_size(Some(8), RequestMismatchMitigationStrategy::PayMoreUpgrade).unwrap(), ImageBatchSize::Four);
      assert_eq!(plan_batch_size(Some(8), RequestMismatchMitigationStrategy::PayLessDowngrade).unwrap(), ImageBatchSize::Four);
      assert!(matches!(
        plan_batch_size(Some(8), RequestMismatchMitigationStrategy::ErrorOut),
        Err(ArtcraftRouterError::Client(ClientError::ModelDoesNotSupportOption { field: "image_batch_count", .. })),
      ));
      assert!(matches!(
        plan_batch_size(Some(0), RequestMismatchMitigationStrategy::PayMoreUpgrade),
        Err(ArtcraftRouterError::Client(ClientError::UserRequestedZeroGenerations)),
      ));
    }

    #[test]
    fn reference_limits() {
      assert!(check_reference_limit(4, 4, "x").is_ok());
      assert!(matches!(
        check_reference_limit(5, 4, "x"),
        Err(ArtcraftRouterError::Client(ClientError::ModelDoesNotSupportOption { field: "image_inputs", .. })),
      ));
    }
  }

  mod resolution_snapping {
    use super::*;

    #[test]
    fn exact_tiers_pass_through_under_every_strategy() {
      for strategy in [RequestMismatchMitigationStrategy::ErrorOut, RequestMismatchMitigationStrategy::PayMoreUpgrade, RequestMismatchMitigationStrategy::PayLessDowngrade] {
        assert_eq!(snap_resolution(Some(RouterResolution::TwoK), NANO_BANANA_PRO_TIERS, NanoBananaProResolution::OneK, strategy).unwrap(), NanoBananaProResolution::TwoK);
      }
    }

    #[test]
    fn none_uses_the_default() {
      assert_eq!(snap_resolution(None, NANO_BANANA_PRO_TIERS, NanoBananaProResolution::TwoK, RequestMismatchMitigationStrategy::ErrorOut).unwrap(), NanoBananaProResolution::TwoK);
    }

    #[test]
    fn upgrade_takes_the_next_tier_up_or_the_top() {
      let strategy = RequestMismatchMitigationStrategy::PayMoreUpgrade;
      // 3K sits between 2K and 4K → 4K.
      assert_eq!(snap_resolution(Some(RouterResolution::ThreeK), NANO_BANANA_PRO_TIERS, NanoBananaProResolution::OneK, strategy).unwrap(), NanoBananaProResolution::FourK);
      // Sub-1K tiers → 1K.
      assert_eq!(snap_resolution(Some(RouterResolution::HalfK), NANO_BANANA_PRO_TIERS, NanoBananaProResolution::OneK, strategy).unwrap(), NanoBananaProResolution::OneK);
      assert_eq!(snap_resolution(Some(RouterResolution::SevenTwentyP), NANO_BANANA_PRO_TIERS, NanoBananaProResolution::OneK, strategy).unwrap(), NanoBananaProResolution::OneK);
      // 1080p sits between 1K and 2K → 2K.
      assert_eq!(snap_resolution(Some(RouterResolution::TenEightyP), NANO_BANANA_PRO_TIERS, NanoBananaProResolution::OneK, strategy).unwrap(), NanoBananaProResolution::TwoK);
    }

    #[test]
    fn downgrade_takes_the_next_tier_down_or_the_bottom() {
      let strategy = RequestMismatchMitigationStrategy::PayLessDowngrade;
      assert_eq!(snap_resolution(Some(RouterResolution::ThreeK), NANO_BANANA_PRO_TIERS, NanoBananaProResolution::OneK, strategy).unwrap(), NanoBananaProResolution::TwoK);
      assert_eq!(snap_resolution(Some(RouterResolution::HalfK), NANO_BANANA_PRO_TIERS, NanoBananaProResolution::OneK, strategy).unwrap(), NanoBananaProResolution::OneK);
      assert_eq!(snap_resolution(Some(RouterResolution::TenEightyP), NANO_BANANA_PRO_TIERS, NanoBananaProResolution::OneK, strategy).unwrap(), NanoBananaProResolution::OneK);
    }

    #[test]
    fn error_out_refuses_unsupported_tiers() {
      let err = snap_resolution(Some(RouterResolution::ThreeK), NANO_BANANA_PRO_TIERS, NanoBananaProResolution::OneK, RequestMismatchMitigationStrategy::ErrorOut).unwrap_err();
      assert!(matches!(err, ArtcraftRouterError::Client(ClientError::ModelDoesNotSupportOption { field: "resolution", .. })));
    }

    #[test]
    fn ranks_interleave_image_and_video_tiers() {
      let ordered = [
        RouterResolution::HalfK, RouterResolution::FourEightyP, RouterResolution::SevenTwentyP, RouterResolution::OneK,
        RouterResolution::TenEightyP, RouterResolution::TwoK, RouterResolution::ThreeK, RouterResolution::FourK,
      ];
      assert!(ordered.windows(2).all(|pair| resolution_rank(pair[0]) < resolution_rank(pair[1])));
    }
  }

  mod aspect_ratio_snapping {
    use super::*;

    #[test]
    fn nano_banana_maps_every_router_ratio() {
      use NanoBananaAspectRatio as Ar;
      let cases = [
        (RouterAspectRatio::Square, Ar::Square1x1),
        (RouterAspectRatio::SquareHd, Ar::Square1x1),
        (RouterAspectRatio::WideThreeByTwo, Ar::Landscape3x2),
        (RouterAspectRatio::WideFourByThree, Ar::Landscape4x3),
        (RouterAspectRatio::WideFiveByFour, Ar::Landscape5x4),
        (RouterAspectRatio::WideSixteenByNine, Ar::Landscape16x9),
        (RouterAspectRatio::Wide, Ar::Landscape16x9),
        (RouterAspectRatio::WideTwentyOneByNine, Ar::Landscape21x9),
        (RouterAspectRatio::TallTwoByThree, Ar::Portrait2x3),
        (RouterAspectRatio::TallThreeByFour, Ar::Portrait3x4),
        (RouterAspectRatio::TallFourByFive, Ar::Portrait4x5),
        (RouterAspectRatio::TallNineBySixteen, Ar::Portrait9x16),
        (RouterAspectRatio::Tall, Ar::Portrait9x16),
        // 9:21 isn't offered; 9:16 is the closest.
        (RouterAspectRatio::TallNineByTwentyOne, Ar::Portrait9x16),
        (RouterAspectRatio::Auto, Ar::Auto),
        (RouterAspectRatio::Auto2k, Ar::Auto),
      ];
      for (requested, expected) in cases {
        assert_eq!(plan_nano_banana_aspect_ratio(Some(requested), false), expected, "{requested:?}");
      }
      assert_eq!(plan_nano_banana_aspect_ratio(None, false), Ar::Landscape16x9);
      assert_eq!(plan_nano_banana_aspect_ratio(None, true), Ar::Auto);
    }

    #[test]
    fn gpt_image_2_snaps_the_missing_ratios() {
      use GptImage2AspectRatio as Ar;
      assert_eq!(plan_gpt_image_2_aspect_ratio(Some(RouterAspectRatio::WideFiveByFour), false), Ar::Landscape4x3);
      assert_eq!(plan_gpt_image_2_aspect_ratio(Some(RouterAspectRatio::TallFourByFive), false), Ar::Portrait3x4);
      assert_eq!(plan_gpt_image_2_aspect_ratio(Some(RouterAspectRatio::SquareHd), false), Ar::Square1x1);
      assert_eq!(plan_gpt_image_2_aspect_ratio(Some(RouterAspectRatio::WideTwentyOneByNine), false), Ar::Landscape21x9);
      assert_eq!(plan_gpt_image_2_aspect_ratio(Some(RouterAspectRatio::Auto), false), Ar::Auto);
      assert_eq!(plan_gpt_image_2_aspect_ratio(None, true), Ar::Auto);
      assert_eq!(plan_gpt_image_2_aspect_ratio(None, false), Ar::Landscape16x9);
    }

    #[test]
    fn seedream_has_no_auto_and_snaps_the_rest() {
      use SeedreamAspectRatio as Ar;
      assert_eq!(plan_seedream_aspect_ratio(None, Ar::Square1x1), Ar::Square1x1);
      assert_eq!(plan_seedream_aspect_ratio(Some(RouterAspectRatio::Auto), Ar::Landscape16x9), Ar::Landscape16x9);
      assert_eq!(plan_seedream_aspect_ratio(Some(RouterAspectRatio::Auto4k), Ar::Landscape16x9), Ar::Landscape16x9);
      assert_eq!(plan_seedream_aspect_ratio(Some(RouterAspectRatio::WideFiveByFour), Ar::Square1x1), Ar::Landscape4x3);
      assert_eq!(plan_seedream_aspect_ratio(Some(RouterAspectRatio::TallFourByFive), Ar::Square1x1), Ar::Portrait3x4);
      assert_eq!(plan_seedream_aspect_ratio(Some(RouterAspectRatio::TallNineByTwentyOne), Ar::Square1x1), Ar::Portrait9x16);
      assert_eq!(plan_seedream_aspect_ratio(Some(RouterAspectRatio::WideThreeByTwo), Ar::Square1x1), Ar::Landscape3x2);
    }

    #[test]
    fn seedream_auto_ratios_carry_a_resolution() {
      assert!(matches!(seedream_auto_resolution_hint(Some(RouterAspectRatio::Auto2k)), Some(RouterResolution::TwoK)));
      assert!(matches!(seedream_auto_resolution_hint(Some(RouterAspectRatio::Auto3k)), Some(RouterResolution::ThreeK)));
      assert!(matches!(seedream_auto_resolution_hint(Some(RouterAspectRatio::Auto4k)), Some(RouterResolution::FourK)));
      assert!(seedream_auto_resolution_hint(Some(RouterAspectRatio::Auto)).is_none());
      assert!(seedream_auto_resolution_hint(None).is_none());
    }
  }
}
