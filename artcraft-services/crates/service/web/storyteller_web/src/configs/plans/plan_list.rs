use std::collections::{HashMap, HashSet};

use once_cell::sync::Lazy;

use tts_common::priority::{FAKEYOU_ANONYMOUS_PRIORITY_LEVEL, FAKEYOU_LOGGED_IN_PRIORITY_LEVEL};

use crate::configs::plans::plan::Plan;
use crate::configs::plans::plan::PlanBuilder;
use crate::configs::plans::plan_category::PlanCategory;

/// The plan for free logged-out users.
/// This is declared at the top level because it may need to be referenced directly.
pub static FREE_LOGGED_OUT_PLAN : Lazy<Plan> = Lazy::new(|| {
    PlanBuilder::new("free_logged_out")
        .is_synthetic_plan(true)
        .plan_category(PlanCategory::Free)
        .tts_base_priority_level(FAKEYOU_ANONYMOUS_PRIORITY_LEVEL)
        .tts_max_character_length(1024)
        .web_vc_base_priority_level(FAKEYOU_ANONYMOUS_PRIORITY_LEVEL)
        .web_vc_requires_frontend_keepalive(true)
        .lipsync_requires_frontend_keepalive(true)
        .build()
});

/// A plan for free logged-out users on their first few attempts at using the website.
/// This is declared at the top level because it may need to be referenced directly.
pub static FREE_LOGGED_OUT_FIRST_TRY_PLAN : Lazy<Plan> = Lazy::new(|| {
    PlanBuilder::new("free_logged_out_first_try")
        .is_synthetic_plan(true)
        .plan_category(PlanCategory::Free)
        .tts_base_priority_level(FAKEYOU_LOGGED_IN_PRIORITY_LEVEL) // NB: Same as logged-in free users.
        .tts_max_character_length(1024)
        .web_vc_base_priority_level(FAKEYOU_LOGGED_IN_PRIORITY_LEVEL) // NB: Same as logged-in free users.
        .web_vc_requires_frontend_keepalive(true)
        .lipsync_requires_frontend_keepalive(true)
        .build()
});

/// The plan for free logged-in users.
/// This is declared at the top level because it may need to be referenced directly.
pub static FREE_LOGGED_IN_PLAN : Lazy<Plan> = Lazy::new(|| {
    PlanBuilder::new("free_logged_in")
        .is_synthetic_plan(true)
        .plan_category(PlanCategory::Free)
        .tts_base_priority_level(FAKEYOU_LOGGED_IN_PRIORITY_LEVEL)
        .tts_max_character_length(1024)
        .web_vc_base_priority_level(FAKEYOU_LOGGED_IN_PRIORITY_LEVEL)
        .web_vc_requires_frontend_keepalive(true)
        .lipsync_requires_frontend_keepalive(true)
        .build()
});

/// These plans are for users that create models for us but don't pay.
pub static LOYALTY_PLANS : Lazy<HashSet<Plan>> = Lazy::new(|| {
    let mut plans = HashSet::new();

    plans.insert(PlanBuilder::new("fakeyou_contributor")
        .is_synthetic_plan(true)
        .plan_category(PlanCategory::LoyaltyReward)
        .tts_base_priority_level(2)
        .tts_max_character_length(2048)
        .tts_max_duration_seconds(30)
        .web_vc_base_priority_level(2)
        .web_vc_requires_frontend_keepalive(false)
        .lipsync_requires_frontend_keepalive(false)
        .build());

    plans
});

/// These plans do not exist in production!
pub static DEVELOPMENT_PREMIUM_PLANS: Lazy<HashSet<Plan>> = Lazy::new(|| {
    let mut plans = HashSet::new();

    // ========== English plans ==========

    plans.insert(PlanBuilder::new("development_fakeyou_plus")
        .is_development_plan(true)
        .plan_category(PlanCategory::Paid)
        .stripe_product_id("prod_MMxi2J5y69VPbO")
        .stripe_price_id("price_1LeDnKEU5se17MekVr1iYYNf")
        .cost_per_month_dollars(7)
        .tts_base_priority_level(20)
        .tts_max_character_length(2048)
        .tts_max_duration_seconds(30)
        .web_vc_requires_frontend_keepalive(false)
        .lipsync_requires_frontend_keepalive(false)
        .build());

    plans.insert(PlanBuilder::new("development_fakeyou_pro")
        .is_development_plan(true)
        .plan_category(PlanCategory::Paid)
        .stripe_product_id("prod_MScAZa5uk5TfDY")
        .stripe_price_id("price_1LjgwIEU5se17MekzQZUHl9W")
        .cost_per_month_dollars(15)
        .tts_base_priority_level(30)
        .tts_max_character_length(2048)
        .tts_max_duration_seconds(60 * 2)
        .web_vc_requires_frontend_keepalive(false)
        .lipsync_requires_frontend_keepalive(false)
        .build());

    plans.insert(PlanBuilder::new("development_fakeyou_elite")
        .is_development_plan(true)
        .plan_category(PlanCategory::Paid)
        .stripe_product_id("prod_MoFffbqBPt2NG5")
        .stripe_price_id("price_1M4dAbEU5se17MekEmwnee41")
        .cost_per_month_dollars(25)
        .tts_base_priority_level(40)
        .tts_max_character_length(2048)
        .tts_max_duration_seconds(60 * 5)
        .web_vc_requires_frontend_keepalive(false)
        .lipsync_requires_frontend_keepalive(false)
        .build());

    plans
});

/// All paid-for production premium plans in the system.
pub static PRODUCTION_PREMIUM_PLANS: Lazy<HashSet<Plan>> = Lazy::new(|| {
  let mut plans = HashSet::new();

  // ========== All-locale plans ==========

  // Old "plus" price
  plans.insert(PlanBuilder::new("fakeyou_plus")
      .plan_category(PlanCategory::Paid)
      .stripe_product_id("prod_MoLv23HrxPiY7D")
      .stripe_price_id("price_1M4jDCEU5se17MekOaJ92HYX")
      .cost_per_month_dollars(7)
      .tts_base_priority_level(20)
      .tts_max_character_length(2048)
      .tts_max_duration_seconds(30)
      .web_vc_base_priority_level(20)
      .web_vc_requires_frontend_keepalive(false)
      .lipsync_requires_frontend_keepalive(false)
      .build());

  // New "plus" price
  plans.insert(PlanBuilder::new("fakeyou_plus_plan")
      .plan_category(PlanCategory::Paid)
      .stripe_product_id("prod_RvC5qUgLTSLGSD")
      .stripe_price_id("price_1R1Lh8EU5se17MekaUNtWc59")
      .cost_per_month_dollars(12)
      .tts_base_priority_level(20)
      .tts_max_character_length(2048)
      .tts_max_duration_seconds(30)
      .web_vc_base_priority_level(20)
      .web_vc_requires_frontend_keepalive(false)
      .lipsync_requires_frontend_keepalive(false)
      .build());

  // Old "pro" price
  plans.insert(PlanBuilder::new("fakeyou_pro")
        .plan_category(PlanCategory::Paid)
        .stripe_product_id("prod_MoLw8nA6eFxHzc")
        .stripe_price_id("price_1M4jEQEU5se17MeksNfA0EKm")
        .cost_per_month_dollars(15)
        .tts_base_priority_level(30)
        .tts_max_character_length(3072)
        .tts_max_duration_seconds(60 * 2)
        .web_vc_base_priority_level(30)
        .web_vc_requires_frontend_keepalive(false)
        .lipsync_requires_frontend_keepalive(false)
        .can_remove_visual_watermarks(true)
        .build());

  // New "pro" price
  plans.insert(PlanBuilder::new("fakeyou_pro_plan")
      .plan_category(PlanCategory::Paid)
      .stripe_product_id("prod_RvC6nB9rw6yAsI")
      .stripe_price_id("price_1R1LhzEU5se17Mek3JLwN3Gg")
      .cost_per_month_dollars(25)
      .tts_base_priority_level(30)
      .tts_max_character_length(3072)
      .tts_max_duration_seconds(60 * 2)
      .web_vc_base_priority_level(30)
      .web_vc_requires_frontend_keepalive(false)
      .lipsync_requires_frontend_keepalive(false)
      .can_remove_visual_watermarks(true)
      .build());

  // Old "elite" price
  plans.insert(PlanBuilder::new("fakeyou_elite")
        .plan_category(PlanCategory::Paid)
        .stripe_product_id("prod_MoLxQmLA4R24fv")
        .stripe_price_id("price_1M4jFREU5se17Mekc8pQaSKB")
        .cost_per_month_dollars(25)
        .tts_base_priority_level(40)
        .tts_max_character_length(4096)
        .tts_max_duration_seconds(60 * 5)
        .web_vc_base_priority_level(40)
        .web_vc_requires_frontend_keepalive(false)
        .lipsync_requires_frontend_keepalive(false)
        .can_remove_visual_watermarks(true)
        .build());

  // New "elite" price
  plans.insert(PlanBuilder::new("fakeyou_elite_plan")
      .plan_category(PlanCategory::Paid)
      .stripe_product_id("prod_RvC6i84YiAYStd")
      .stripe_price_id("price_1R1LiQEU5se17MekiJdBcsHQ")
      .cost_per_month_dollars(40)
      .tts_base_priority_level(40)
      .tts_max_character_length(4096)
      .tts_max_duration_seconds(60 * 5)
      .web_vc_base_priority_level(40)
      .web_vc_requires_frontend_keepalive(false)
      .lipsync_requires_frontend_keepalive(false)
      .can_remove_visual_watermarks(true)
      .build());

    // ========== Spanish plans ==========

    plans.insert(PlanBuilder::new("fakeyou_basic")
        .plan_category(PlanCategory::Paid)
        .stripe_product_id("prod_MoLyt8qMDmscjr")
        .stripe_price_id("price_1M4jGPEU5se17Mek2FiztNE5")
        .cost_per_month_dollars(3)
        .tts_base_priority_level(10)
        .web_vc_base_priority_level(10)
        .web_vc_requires_frontend_keepalive(false)
        .lipsync_requires_frontend_keepalive(false)
        .tts_max_duration_seconds(30)
        .build());

    plans
});

/// Only free plans
pub static FREE_PLANS_BY_SLUG : Lazy<HashMap<String, Plan>> = Lazy::new(|| {
    let mut plans = HashMap::new();

    fn add_plan(plans: &mut HashMap<String, Plan>, plan: &Plan) {
        plans.insert(plan.plan_slug().to_string(), plan.clone());
    }

    add_plan(&mut plans, &FREE_LOGGED_OUT_PLAN);
    add_plan(&mut plans, &FREE_LOGGED_OUT_FIRST_TRY_PLAN);
    add_plan(&mut plans, &FREE_LOGGED_IN_PLAN);

    plans
});

/// Only loyalty premium plans (non-Stripe, non-PayPal, etc. - internally defined)
pub static LOYALTY_PLANS_BY_SLUG : Lazy<HashMap<String, Plan>> = Lazy::new(|| {
    let mut plans = HashMap::new();

    fn add_plan(plans: &mut HashMap<String, Plan>, plan: &Plan) {
        plans.insert(plan.plan_slug().to_string(), plan.clone());
    }

    LOYALTY_PLANS.iter().for_each(|plan| add_plan(&mut plans, plan));

    plans
});

/// Only development premium plans
pub static DEVELOPMENT_PREMIUM_PLANS_BY_SLUG : Lazy<HashMap<String, Plan>> = Lazy::new(|| {
    let mut plans = HashMap::new();

    fn add_plan(plans: &mut HashMap<String, Plan>, plan: &Plan) {
        plans.insert(plan.plan_slug().to_string(), plan.clone());
    }

    DEVELOPMENT_PREMIUM_PLANS.iter().for_each(|plan| add_plan(&mut plans, plan));

    plans
});

/// Only production premium plans
pub static PRODUCTION_PREMIUM_PLANS_BY_SLUG : Lazy<HashMap<String, Plan>> = Lazy::new(|| {
    let mut plans = HashMap::new();

    fn add_plan(plans: &mut HashMap<String, Plan>, plan: &Plan) {
        plans.insert(plan.plan_slug().to_string(), plan.clone());
    }

    PRODUCTION_PREMIUM_PLANS.iter().for_each(|plan| add_plan(&mut plans, plan));

    plans
});

/// Every single plan in the system: free, paid, loyalty-based, and Stripe development plans.
pub static ALL_PLANS_BY_SLUG : Lazy<HashMap<String, Plan>> = Lazy::new(|| {
    let mut plans = HashMap::new();

    fn add_plan(plans: &mut HashMap<String, Plan>, plan: &Plan) {
        plans.insert(plan.plan_slug().to_string(), plan.clone());
    }

    add_plan(&mut plans, &FREE_LOGGED_OUT_PLAN);
    add_plan(&mut plans, &FREE_LOGGED_OUT_FIRST_TRY_PLAN);
    add_plan(&mut plans, &FREE_LOGGED_IN_PLAN);

    LOYALTY_PLANS.iter().for_each(|plan| add_plan(&mut plans, plan));
    PRODUCTION_PREMIUM_PLANS.iter().for_each(|plan| add_plan(&mut plans, plan));
    DEVELOPMENT_PREMIUM_PLANS.iter().for_each(|plan| add_plan(&mut plans, plan));

    plans
});

/// Every Stripe premium product by its Stripe product id.
pub static PLANS_BY_STRIPE_PRODUCT_ID : Lazy<HashMap<String, Plan>> = Lazy::new(|| {
    let mut plans = HashMap::new();

    ALL_PLANS_BY_SLUG.values()
        .for_each(|plan| {
            if let Some(stripe_product_id) = plan.stripe_product_id() {
                plans.insert(stripe_product_id.to_string(), plan.clone());
            }
        });

    plans
});

/// Every Stripe premium product by its Stripe price id.
pub static PLANS_BY_STRIPE_PRICE_ID : Lazy<HashMap<String, Plan>> = Lazy::new(|| {
    let mut plans = HashMap::new();

    ALL_PLANS_BY_SLUG.values()
        .for_each(|plan| {
            if let Some(stripe_price_id) = plan.stripe_price_id() {
                plans.insert(stripe_price_id.to_string(), plan.clone());
            }
        });

    plans
});

#[cfg(test)]
mod test {
  use speculoos::prelude::*;

  use crate::configs::plans::plan::Plan;
  use crate::configs::plans::plan_category::PlanCategory;
  use crate::configs::plans::plan_list::{ALL_PLANS_BY_SLUG, DEVELOPMENT_PREMIUM_PLANS, DEVELOPMENT_PREMIUM_PLANS_BY_SLUG, FREE_PLANS_BY_SLUG, LOYALTY_PLANS, LOYALTY_PLANS_BY_SLUG, PLANS_BY_STRIPE_PRICE_ID, PLANS_BY_STRIPE_PRODUCT_ID, PRODUCTION_PREMIUM_PLANS, PRODUCTION_PREMIUM_PLANS_BY_SLUG};

  // NB: We're being extremely careful in this test and all those that follow, essentially
    // making ourselves check twice when we make additions, removals, or other changes.
    #[test]
    fn test_number_of_plans_is_expected() {
        assert_eq!(14, ALL_PLANS_BY_SLUG.len());
    }

    #[test]
    fn test_number_of_stripe_production_plans_is_expected() {
        let production_plans = ALL_PLANS_BY_SLUG.values()
            .filter(|plan| !plan.is_development_plan())
            .filter(|plan| !plan.is_synthetic_plan()) // NB: Synthetic plans are production!
            .collect::<Vec<&Plan>>();

        let expected = 7;

        assert_eq!(expected, production_plans.len());
        assert_eq!(expected, PRODUCTION_PREMIUM_PLANS.len());
        assert_eq!(expected, PRODUCTION_PREMIUM_PLANS_BY_SLUG.len());
    }

    #[test]
    fn test_number_of_stripe_development_plans_is_expected() {
        let development_plans = ALL_PLANS_BY_SLUG.values()
            .filter(|plan| plan.is_development_plan())
            .collect::<Vec<&Plan>>();

        let expected = 3;

        assert_eq!(expected, development_plans.len());
        assert_eq!(expected, DEVELOPMENT_PREMIUM_PLANS.len());
        assert_eq!(expected, DEVELOPMENT_PREMIUM_PLANS_BY_SLUG.len());
    }

    #[test]
    fn test_number_of_loyalty_plans_is_expected() {
        let expected = 1;

        assert_eq!(expected, LOYALTY_PLANS.len());
        assert_eq!(expected, LOYALTY_PLANS_BY_SLUG.len());
    }


    #[test]
    fn test_plans_by_stripe_product_id_are_expected() {
        let product_ids = PLANS_BY_STRIPE_PRODUCT_ID.keys()
            .map(|product_id| product_id.to_string())
            .collect::<Vec<String>>();

        let expected_product_ids = vec![
            "prod_MMxi2J5y69VPbO".to_string(), // NB: Development
            "prod_MScAZa5uk5TfDY".to_string(), // NB: Development
            "prod_MoFffbqBPt2NG5".to_string(), // NB: Development
            "prod_MoLv23HrxPiY7D".to_string(), // NB: Production
            "prod_MoLw8nA6eFxHzc".to_string(), // NB: Production
            "prod_MoLxQmLA4R24fv".to_string(), // NB: Production
            "prod_MoLyt8qMDmscjr".to_string(), // NB: Production
            "prod_RvC5qUgLTSLGSD".to_string(), // NB: Production - new "plus" product
            "prod_RvC6nB9rw6yAsI".to_string(), // NB: Production - new "pro" product
            "prod_RvC6i84YiAYStd".to_string(), // NB: Production - new "elite" product
        ];

        assert_that(&product_ids).contains_all_of(&expected_product_ids.iter());
        assert_eq!(product_ids.len(), expected_product_ids.len());
    }

    #[test]
    fn test_plans_by_stripe_price_id_are_expected() {
        let price_ids = PLANS_BY_STRIPE_PRICE_ID.keys()
            .map(|price_id| price_id.to_string())
            .collect::<Vec<String>>();

        let expected_price_ids = vec![
            "price_1LjgwIEU5se17MekzQZUHl9W".to_string(), // NB: Development
            "price_1LeDnKEU5se17MekVr1iYYNf".to_string(), // NB: Development
            "price_1M4dAbEU5se17MekEmwnee41".to_string(), // NB: Development
            "price_1M4jDCEU5se17MekOaJ92HYX".to_string(), // NB: Production
            "price_1M4jEQEU5se17MeksNfA0EKm".to_string(), // NB: Production
            "price_1M4jFREU5se17Mekc8pQaSKB".to_string(), // NB: Production
            "price_1M4jGPEU5se17Mek2FiztNE5".to_string(), // NB: Production
            "price_1R1Lh8EU5se17MekaUNtWc59".to_string(), // NB: Production - new "plus" price
            "price_1R1LhzEU5se17Mek3JLwN3Gg".to_string(), // NB: Production - new "pro" price
            "price_1R1LiQEU5se17MekiJdBcsHQ".to_string(), // NB: Production - new "elite" price
        ];

        assert_that(&price_ids).contains_all_of(&expected_price_ids.iter());
        assert_eq!(price_ids.len(), expected_price_ids.len());
    }

    #[test]
    fn test_tts_base_priority_levels_are_expected() {
        // Free
        assert_eq!(0, ALL_PLANS_BY_SLUG.get("free_logged_out").unwrap().tts_base_priority_level());
        assert_eq!(1, ALL_PLANS_BY_SLUG.get("free_logged_out_first_try").unwrap().tts_base_priority_level());
        assert_eq!(1, ALL_PLANS_BY_SLUG.get("free_logged_in").unwrap().tts_base_priority_level());

        // Loyalty-based
        assert_eq!(2, ALL_PLANS_BY_SLUG.get("fakeyou_contributor").unwrap().tts_base_priority_level());

        // Premium (Production)
        assert_eq!(10, ALL_PLANS_BY_SLUG.get("fakeyou_basic").unwrap().tts_base_priority_level());
        assert_eq!(20, ALL_PLANS_BY_SLUG.get("fakeyou_plus").unwrap().tts_base_priority_level());
        assert_eq!(30, ALL_PLANS_BY_SLUG.get("fakeyou_pro").unwrap().tts_base_priority_level());
        assert_eq!(40, ALL_PLANS_BY_SLUG.get("fakeyou_elite").unwrap().tts_base_priority_level());

        // Premium (Development/Test)
        assert_eq!(20, ALL_PLANS_BY_SLUG.get("development_fakeyou_plus").unwrap().tts_base_priority_level());
        assert_eq!(30, ALL_PLANS_BY_SLUG.get("development_fakeyou_pro").unwrap().tts_base_priority_level());
        assert_eq!(40, ALL_PLANS_BY_SLUG.get("development_fakeyou_elite").unwrap().tts_base_priority_level());
    }

    #[test]
    fn test_tts_max_durations_are_expected() {
        // Free
        assert_eq!(12, ALL_PLANS_BY_SLUG.get("free_logged_out").unwrap().tts_max_duration().num_seconds());
        assert_eq!(12, ALL_PLANS_BY_SLUG.get("free_logged_out_first_try").unwrap().tts_max_duration().num_seconds());
        assert_eq!(12, ALL_PLANS_BY_SLUG.get("free_logged_in").unwrap().tts_max_duration().num_seconds());

        // Loyalty-based
        assert_eq!(30, ALL_PLANS_BY_SLUG.get("fakeyou_contributor").unwrap().tts_max_duration().num_seconds());

        // Premium (Production)
        assert_eq!(30, ALL_PLANS_BY_SLUG.get("fakeyou_basic").unwrap().tts_max_duration().num_seconds());
        assert_eq!(30, ALL_PLANS_BY_SLUG.get("fakeyou_plus").unwrap().tts_max_duration().num_seconds());
        assert_eq!(120, ALL_PLANS_BY_SLUG.get("fakeyou_pro").unwrap().tts_max_duration().num_seconds());
        assert_eq!(300, ALL_PLANS_BY_SLUG.get("fakeyou_elite").unwrap().tts_max_duration().num_seconds());

        // Premium (Development/Test)
        assert_eq!(30, ALL_PLANS_BY_SLUG.get("development_fakeyou_plus").unwrap().tts_max_duration().num_seconds());
        assert_eq!(120, ALL_PLANS_BY_SLUG.get("development_fakeyou_pro").unwrap().tts_max_duration().num_seconds());
        assert_eq!(300, ALL_PLANS_BY_SLUG.get("development_fakeyou_elite").unwrap().tts_max_duration().num_seconds());
    }

    // =================== TYPICAL FEATURE BEHAVIORS (SUBJECT TO CHANGE) =================== //

    #[test]
    fn test_assert_web_vc_keepalive_forced() {
        FREE_PLANS_BY_SLUG.iter().for_each(|(price_id, plan)| {
            assert!(plan.web_vc_requires_frontend_keepalive());
            assert!(plan.lipsync_requires_frontend_keepalive());
        });
    }

    #[test]
    fn test_assert_web_vc_no_keepalive() {
        LOYALTY_PLANS_BY_SLUG.iter().for_each(|(price_id, plan)| {
            assert!(!plan.web_vc_requires_frontend_keepalive());
            assert!(!plan.lipsync_requires_frontend_keepalive());
        });
        PRODUCTION_PREMIUM_PLANS_BY_SLUG.iter().for_each(|(price_id, plan)| {
            assert!(!plan.web_vc_requires_frontend_keepalive());
            assert!(!plan.lipsync_requires_frontend_keepalive());
        });
    }

    // =================== THE FOLLOWING TESTS SHOULD NOT NEED TO CHANGE MUCH =================== //

    #[test]
    fn test_assert_all_production_stripe_plans_are_paid() {
        PRODUCTION_PREMIUM_PLANS.iter().for_each(|plan| {
            assert_eq!(plan.plan_category(), PlanCategory::Paid);
        });
        PRODUCTION_PREMIUM_PLANS_BY_SLUG.iter().for_each(|(_, plan)| {
            assert_eq!(plan.plan_category(), PlanCategory::Paid);
        });
    }

    #[test]
    fn test_assert_all_production_stripe_plans_are_non_synthetic() {
        PRODUCTION_PREMIUM_PLANS.iter().for_each(|plan| {
            assert!(!plan.is_synthetic_plan());
        });
        PRODUCTION_PREMIUM_PLANS_BY_SLUG.iter().for_each(|(_, plan)| {
            assert!(!plan.is_synthetic_plan());
        });
    }

    #[test]
    fn test_assert_all_development_stripe_plans_are_marked_development() {
        DEVELOPMENT_PREMIUM_PLANS.iter().for_each(|plan| {
            assert!(plan.is_development_plan());
        });
        DEVELOPMENT_PREMIUM_PLANS_BY_SLUG.iter().for_each(|(_, plan)| {
            assert!(plan.is_development_plan());
        });
    }

    #[test]
    fn test_assert_all_development_stripe_plans_are_non_synthetic() {
        DEVELOPMENT_PREMIUM_PLANS.iter().for_each(|plan| {
            assert!(!plan.is_synthetic_plan());
        });
        DEVELOPMENT_PREMIUM_PLANS_BY_SLUG.iter().for_each(|(_, plan)| {
            assert!(!plan.is_synthetic_plan());
        });
    }

    #[test]
    fn test_assert_all_loyalty_plans_are_synthetic() {
        LOYALTY_PLANS.iter().for_each(|plan| {
            assert!(plan.is_synthetic_plan());
        });
    }

    #[test]
    fn test_assert_correct_key_used_for_plans_indexed_by_slug() {
        ALL_PLANS_BY_SLUG.iter().for_each(|(slug, plan)| {
            assert_eq!(slug, plan.plan_slug());
        });
        PRODUCTION_PREMIUM_PLANS_BY_SLUG.iter().for_each(|(slug, plan)| {
            assert_eq!(slug, plan.plan_slug());
        });
        DEVELOPMENT_PREMIUM_PLANS_BY_SLUG.iter().for_each(|(slug, plan)| {
            assert_eq!(slug, plan.plan_slug());
        });
        LOYALTY_PLANS_BY_SLUG.iter().for_each(|(slug, plan)| {
            assert_eq!(slug, plan.plan_slug());
        });
    }

    #[test]
    fn test_assert_correct_key_for_plans_indexed_by_product_id() {
        PLANS_BY_STRIPE_PRODUCT_ID.iter().for_each(|(product_id, plan)| {
            assert_eq!(product_id, plan.stripe_product_id().unwrap());
        });
    }

    #[test]
    fn test_assert_correct_key_for_plans_indexed_by_price_id() {
        PLANS_BY_STRIPE_PRICE_ID.iter().for_each(|(price_id, plan)| {
            assert_eq!(price_id, plan.stripe_price_id().unwrap());
        });
    }

    #[test]
    fn test_assert_stripe_plans_are_non_synthetic() {
        PLANS_BY_STRIPE_PRICE_ID.iter().for_each(|(price_id, plan)| {
            assert!(!plan.is_synthetic_plan());
        });
        PLANS_BY_STRIPE_PRODUCT_ID.iter().for_each(|(price_id, plan)| {
            assert!(!plan.is_synthetic_plan());
        });
    }
}
