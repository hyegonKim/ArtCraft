use chrono::Duration;

use crate::configs::plans::plan_category::PlanCategory;

const TTS_DEFAULT_PRIORITY_LEVEL : u8 = 0;
const TTS_DEFAULT_DURATION_SECONDS : i64 = 12;

const VC_DEFAULT_PRIORITY_LEVEL : u8 = 0;

const VC_DEFAULT_MAX_CONCURRENT_MODELS : u32 = 5;

const VC_DEFAULT_PRE_RECORDED_TIME_LIMIT_SECONDS : i64 = 20;
const VC_DEFAULT_REAL_TIME_TIME_LIMIT_SECONDS : i64 = 20;

const W2L_DEFAULT_TIME_LIMIT_SECONDS : i64 = 20;


/// A Plan is either a free or premium plan.
/// Each plan corresponds to a certain level of service.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Plan {
    /// Name of the plan in URL-friendly "slug" form (human readable, no spaces, underscores),
    /// eg. "plan_1", "en_basic", etc.
    plan_slug: String,

    /// Whether the plan is free, paid, or loyalty-based
    plan_category: PlanCategory,

    /// Non-authoritative cost of the plan per month in USD equivalent.
    /// This is mostly for documentation. Stripe will remain the source
    /// of truth.
    cost_per_month_dollars: Option<u32>,

    /// For lookup against Stripe product ID
    stripe_product_id: Option<String>,

    /// For lookup against Stripe price ID
    stripe_price_id: Option<String>,

    /// Whether this plan is only active in development
    is_development_plan: bool,

    /// Whether this plan does not exist in Stripe and only exists as defined by this server.
    is_synthetic_plan: bool,

    // ========== Features for lipsync (SadTalker, not Wav2Lip) ==========

    lipsync_requires_frontend_keepalive: bool,

    can_remove_visual_watermarks: bool,

    // ========== Features for Videofilters ==========
    rerender_requires_frontend_keepalive: bool,

    // ========== Features for Mocap ==========
    mocapnet_requires_frontend_keepalive: bool,

    // ========== Features for TTS ==========

    tts_base_priority_level: u8,
    tts_max_duration: Duration,
    tts_max_character_length: usize,
    tts_can_generate_mp3: bool, // TODO: This feature doesn't exist yet.
    tts_can_upload_private_models: bool,
    tts_can_share_private_models: bool,

    // ========== Features for Web Voice Conversion ==========

    web_vc_base_priority_level: u8,

    web_vc_requires_frontend_keepalive: bool,

    // ========== Features for Real Time Voice Conversion ==========

    vc_max_concurrent_models: u32,

    vc_pre_recorded_time_limit: Duration,
    vc_pre_recorded_time_is_unlimited: bool,

    vc_real_time_time_limit: Duration,
    vc_real_time_time_is_unlimited: bool,

    // ========== Features for W2L ==========

    w2l_max_duration: Duration,
    w2l_can_turn_off_watermark: bool,
}

impl Plan {
    pub fn from_builder(builder: &PlanBuilder) -> Self {
        Self {
            plan_slug: builder.plan_slug.clone(),
            plan_category: builder.plan_category,
            cost_per_month_dollars: builder.cost_per_month_dollars,
            stripe_product_id: builder.stripe_product_id.clone(),
            stripe_price_id: builder.stripe_price_id.clone(),
            is_development_plan: builder.is_development_plan,
            is_synthetic_plan: builder.is_synthetic_plan,
            lipsync_requires_frontend_keepalive: builder.lipsync_requires_frontend_keepalive,
            rerender_requires_frontend_keepalive: builder.rerender_requires_frontend_keepalive,
            mocapnet_requires_frontend_keepalive: builder.mocapnet_requires_frontend_keepalive,
            can_remove_visual_watermarks: builder.can_remove_visual_watermarks,
            tts_base_priority_level: builder.tts_base_priority_level,
            tts_max_duration: builder.tts_max_duration,
            tts_max_character_length: builder.tts_max_character_length,
            tts_can_generate_mp3: builder.tts_can_generate_mp3,
            tts_can_upload_private_models: builder.tts_can_upload_private_models,
            tts_can_share_private_models: builder.tts_can_share_private_models,
            web_vc_base_priority_level: builder.web_vc_base_priority_level,
            web_vc_requires_frontend_keepalive: builder.web_vc_requires_frontend_keepalive,
            vc_max_concurrent_models: builder.vc_max_concurrent_models,
            vc_pre_recorded_time_limit: builder.vc_pre_recorded_time_limit,
            vc_pre_recorded_time_is_unlimited: builder.vc_pre_recorded_time_is_unlimited,
            vc_real_time_time_limit: builder.vc_real_time_time_limit,
            vc_real_time_time_is_unlimited: builder.vc_real_time_time_is_unlimited,
            w2l_max_duration: builder.w2l_max_duration,
            w2l_can_turn_off_watermark: builder.w2l_can_turn_off_watermark,
        }
    }

    pub fn plan_slug(&self) -> &str {
        &self.plan_slug
    }

    pub fn plan_category(&self) -> PlanCategory {
        self.plan_category
    }

    pub fn stripe_product_id(&self) -> Option<&str> {
        self.stripe_product_id.as_deref()
    }

    pub fn stripe_price_id(&self) -> Option<&str> {
        self.stripe_price_id.as_deref()
    }

    /// The plan is only valid in "development" Stripe.
    /// These plans should never be used on the production cluster.
    pub fn is_development_plan(&self) -> bool {
        self.is_development_plan
    }

    /// The plan is a valid Stripe plan in production (not test/development).
    pub fn is_production_plan(&self) -> bool {
        !self.is_development_plan
    }

    /// The plan is "synthetic", ie. does not exist in Stripe.
    pub fn is_synthetic_plan(&self) -> bool {
        self.is_synthetic_plan
    }

    pub fn lipsync_requires_frontend_keepalive(&self) -> bool {
        self.lipsync_requires_frontend_keepalive
    }

    pub fn rerender_requires_frontent_keepalive(&self) -> bool {
        self.rerender_requires_frontend_keepalive
    }

    pub fn mocapnet_requires_frontend_keepalive(&self) -> bool {
        self.mocapnet_requires_frontend_keepalive
    }

    pub fn workflow_requires_frontend_keepalive(&self) -> bool {
        self.mocapnet_requires_frontend_keepalive
    }

    pub fn can_remove_visual_watermarks(&self) -> bool {
        self.can_remove_visual_watermarks
    }

    pub fn tts_base_priority_level(&self) -> u8 {
        self.tts_base_priority_level
    }

    pub fn tts_max_duration(&self) -> Duration {
        self.tts_max_duration
    }

    pub fn tts_max_duration_seconds(&self) -> i32 {
        self.tts_max_duration.num_seconds() as i32
    }

    pub fn tts_max_character_length(&self) -> usize {
        self.tts_max_character_length
    }

    pub fn web_vc_base_priority_level(&self) -> u8 {
        self.web_vc_base_priority_level
    }

    pub fn web_vc_requires_frontend_keepalive(&self) -> bool {
        self.web_vc_requires_frontend_keepalive
    }
}

/// Builder pattern for Plans.
#[derive(Clone)]
pub struct PlanBuilder {
    plan_slug: String,
    plan_category: PlanCategory,
    cost_per_month_dollars: Option<u32>,
    stripe_product_id: Option<String>,
    stripe_price_id: Option<String>,
    is_development_plan: bool,
    is_synthetic_plan: bool,

    // ========== Features for lipsync (SadTalker, not Wav2Lip) ==========

    lipsync_requires_frontend_keepalive: bool,

    can_remove_visual_watermarks: bool,

    // ========== Features for Videofilters ==========
    rerender_requires_frontend_keepalive: bool,

    // ========== Features for TTS ==========

    tts_base_priority_level: u8,
    tts_max_duration: Duration,
    tts_max_character_length: usize,
    tts_can_generate_mp3: bool,
    tts_can_upload_private_models: bool,
    tts_can_share_private_models: bool,

    // ========== Features for Web Voice Conversion ==========

    web_vc_base_priority_level: u8,

    web_vc_requires_frontend_keepalive: bool,

    // ========== Features for Real Time Voice Conversion ==========

    vc_max_concurrent_models: u32,

    vc_pre_recorded_time_limit: Duration,
    vc_pre_recorded_time_is_unlimited: bool,

    vc_real_time_time_limit: Duration,
    vc_real_time_time_is_unlimited: bool,

    // ========== Features for W2L ==========

    w2l_max_duration: Duration,
    w2l_can_turn_off_watermark: bool,
    mocapnet_requires_frontend_keepalive: bool,
}

impl PlanBuilder {
    pub fn new(plan_slug: &str) -> Self {
        // NB: Not using default() since that seems dangerous when we want to be explicit.
        PlanBuilder {
            plan_slug: plan_slug.to_string(),
            plan_category : PlanCategory::Free,
            cost_per_month_dollars: None,
            stripe_product_id: None,
            stripe_price_id: None,
            is_development_plan: false,
            is_synthetic_plan: false,

            // Lipsync (SadTalker, not Wav2Lip)
            lipsync_requires_frontend_keepalive: true,
            can_remove_visual_watermarks: false,

            // Videofilters
            rerender_requires_frontend_keepalive: true,

            // Mocapnet
            mocapnet_requires_frontend_keepalive: true,

            // TTS
            tts_base_priority_level : TTS_DEFAULT_PRIORITY_LEVEL,
            tts_max_duration: Duration::seconds(TTS_DEFAULT_DURATION_SECONDS),
            // NB: 1024 was the global (all plan) character limit until 2023-05-15 !
            tts_max_character_length: 1024,
            tts_can_generate_mp3 : false,
            tts_can_upload_private_models: false,
            tts_can_share_private_models : false,

            // VC (WEB)
            web_vc_base_priority_level: VC_DEFAULT_PRIORITY_LEVEL,
            web_vc_requires_frontend_keepalive: true,

            // VC (REALTIME)
            vc_max_concurrent_models: VC_DEFAULT_MAX_CONCURRENT_MODELS,
            vc_pre_recorded_time_limit: Duration::seconds(VC_DEFAULT_PRE_RECORDED_TIME_LIMIT_SECONDS),
            vc_pre_recorded_time_is_unlimited: false,
            vc_real_time_time_limit: Duration::seconds(VC_DEFAULT_REAL_TIME_TIME_LIMIT_SECONDS),
            vc_real_time_time_is_unlimited: false,

            // W2L
            w2l_max_duration: Duration::seconds(W2L_DEFAULT_TIME_LIMIT_SECONDS),
            w2l_can_turn_off_watermark: false,
        }
    }

    pub fn build(&self) -> Plan {
        Plan::from_builder(self)
    }

    pub fn plan_category(mut self, value: PlanCategory) -> Self {
        self.plan_category = value;
        self
    }

    pub fn cost_per_month_dollars(mut self, value: u32) -> Self {
        self.cost_per_month_dollars = Some(value);
        self
    }

    pub fn stripe_product_id(mut self, value: &str) -> Self {
        self.stripe_product_id = Some(value.to_string());
        self
    }

    pub fn stripe_price_id(mut self, value: &str) -> Self {
        self.stripe_price_id = Some(value.to_string());
        self
    }

    pub fn is_development_plan(mut self, value: bool) -> Self {
        self.is_development_plan = value;
        self
    }

    pub fn is_synthetic_plan(mut self, value: bool) -> Self {
        self.is_synthetic_plan = value;
        self
    }

    pub fn lipsync_requires_frontend_keepalive(mut self, value: bool) -> Self {
        self.lipsync_requires_frontend_keepalive = value;
        self
    }

    pub fn rerender_requires_frontend_keepalive(mut self, value: bool) -> Self {
        self.rerender_requires_frontend_keepalive = value;
        self
    }

    pub fn can_remove_visual_watermarks(mut self, value: bool) -> Self {
        self.can_remove_visual_watermarks = value;
        self
    }

    pub fn tts_base_priority_level(mut self, value: u8) -> Self {
        self.tts_base_priority_level = value;
        self
    }

    pub fn tts_max_duration_seconds(mut self, value: i64) -> Self {
        self.tts_max_duration = Duration::seconds(value);
        self
    }

    pub fn tts_max_character_length(mut self, value: usize) -> Self {
        self.tts_max_character_length = value;
        self
    }

    pub fn tts_can_generate_mp3(mut self, value: bool) -> Self {
        self.tts_can_generate_mp3 = value;
        self
    }

    pub fn tts_can_upload_private_models(mut self, value: bool) -> Self {
        self.tts_can_upload_private_models = value;
        self
    }

    pub fn tts_can_share_private_models(mut self, value: bool) -> Self {
        self.tts_can_share_private_models = value;
        self
    }

    pub fn web_vc_base_priority_level(mut self, value: u8) -> Self {
        self.web_vc_base_priority_level = value;
        self
    }

    pub fn web_vc_requires_frontend_keepalive(mut self, value: bool) -> Self {
        self.web_vc_requires_frontend_keepalive = value;
        self
    }

    pub fn vc_max_concurrent_models(mut self, value: u32) -> Self {
        self.vc_max_concurrent_models = value;
        self
    }

    pub fn vc_pre_recorded_time_limit_seconds(mut self, value: i64) -> Self {
        self.vc_pre_recorded_time_limit = Duration::seconds(value);
        self
    }

    pub fn vc_pre_recorded_time_is_unlimited(mut self, value: bool) -> Self {
        self.vc_pre_recorded_time_is_unlimited = value;
        if value {
            self.vc_pre_recorded_time_limit = Duration::seconds(60 * 60 * 24 * 7);
        }
        self
    }

    pub fn vc_real_time_time_limit_seconds(mut self, value: i64) -> Self {
        self.vc_real_time_time_limit = Duration::seconds(value);
        self
    }

    pub fn vc_real_time_time_is_unlimited(mut self, value: bool) -> Self {
        self.vc_real_time_time_is_unlimited = value;
        if value {
            self.vc_real_time_time_limit = Duration::seconds(60 * 60 * 24 * 7);
        }
        self
    }

    pub fn w2l_time_limit_seconds(mut self, value: i64) -> Self {
        self.w2l_max_duration = Duration::seconds(value);
        self
    }

    pub fn w2l_can_turn_off_watermark(mut self, value: bool) -> Self {
        self.w2l_can_turn_off_watermark = value;
        self
    }
}

// TODO: Add tests
