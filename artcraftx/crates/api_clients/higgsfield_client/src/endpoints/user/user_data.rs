//! GET `/fnf/user` — the logged-in user's account: plan, credits, workspace,
//! and account status. Also the cheapest way to confirm a session is valid.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::ids::{UserId, WorkspaceId};
use serde::Deserialize;
use serde_json::Value;
use crate::types::string_enum::string_enum;

const PATH: &str = "/fnf/user";

string_enum! {
  /// The subscription plan.
  PlanType {
    Free => "free",
    Basic => "basic",
    Pro => "pro",
    Plus => "plus",
    Ultimate => "ultimate",
    Creator => "creator",
    Enterprise => "enterprise",
  }
}

string_enum! {
  BillingPeriod {
    Monthly => "monthly",
    Yearly => "yearly",
  }
}

pub struct UserDataArgs<'a> {
  pub request: UserDataRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// No parameters; kept for uniformity with the other endpoints.
#[derive(Clone, Debug, Default)]
pub struct UserDataRequest;

/// The fields we rely on. The real payload has dozens more (per-feature
/// credit buckets, consent flags, promo state, ...); they're kept in `extra`
/// rather than typed, since they change often.
#[derive(Clone, Debug, Deserialize)]
pub struct UserDataResponse {
  pub id: UserId,

  #[serde(default)]
  pub email: Option<String>,

  #[serde(default)]
  pub plan_type: Option<PlanType>,

  #[serde(default)]
  pub plan_version: Option<i64>,

  #[serde(default)]
  pub billing_period: Option<BillingPeriod>,

  /// RFC 3339 timestamp.
  #[serde(default)]
  pub plan_ends_at: Option<String>,

  #[serde(default)]
  pub subscription_credits: Option<f64>,

  #[serde(default)]
  pub package_credits: Option<f64>,

  #[serde(default)]
  pub daily_credits: Option<f64>,

  #[serde(default)]
  pub total_plan_credits: Option<f64>,

  /// The plan includes an "unlimited" generation pool.
  #[serde(default)]
  pub has_unlim: bool,

  #[serde(default)]
  pub has_flex_unlim: bool,

  #[serde(default)]
  pub workspace_id: Option<WorkspaceId>,

  /// e.g. `private`.
  #[serde(default)]
  pub workspace_type: Option<String>,

  /// e.g. `owner`.
  #[serde(default)]
  pub workspace_role: Option<String>,

  #[serde(default)]
  pub lang: Option<String>,

  #[serde(default)]
  pub is_paused: bool,

  #[serde(default)]
  pub is_test_user: bool,

  /// RFC 3339 timestamp; set when the account is blocked.
  #[serde(default)]
  pub blocked_at: Option<String>,

  /// RFC 3339 timestamp; set when the account is suspended.
  #[serde(default)]
  pub suspended_at: Option<String>,

  /// Everything not typed above, keyed by field name.
  #[serde(flatten)]
  pub extra: serde_json::Map<String, Value>,
}

impl UserDataResponse {
  /// Whether the account can generate (not blocked, suspended, or paused).
  pub fn is_account_active(&self) -> bool {
    self.blocked_at.is_none() && self.suspended_at.is_none() && !self.is_paused
  }
}

pub async fn user_data(args: UserDataArgs<'_>) -> Result<UserDataResponse, HiggsfieldError> {
  send_json_request(HttpMethod::Get, PATH, args.auth, args.host, None::<&()>).await
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Captured, with the id / email / timestamps scrubbed.
  const USER_RESPONSE: &str = r#"{"id":"user_TESTUSER0000000000000000000","enterprise_new_main":false,"plan_type":"plus","subscription_credits":1200.0,"package_credits":6.0,"soul_credits":0.0,"wan2_5_video_credits":0.0,"text2keyframes_credits":0.0,"face_swap_credits":2.0,"qwen_camera_control_credits":0.0,"character_swap_credits":0.0,"daily_credits":6.0,"total_plan_credits":1200,"billing_period":"monthly","is_cancel_inited":false,"plan_ends_at":"2026-09-30T03:30:59+00:00","next_credit_allocation_at":null,"is_pro_plan_veo3_available":false,"cohort":"tier_1","promo_state":"inactive","has_unlim":true,"veo3_fast_generations_count":0,"auto_publish":false,"lang":"en","is_creator_partner_program":false,"pause_starts_at":null,"pause_resumes_at":null,"is_pause_scheduled":false,"is_paused":false,"is_gift_subscription":false,"workspace_id":"00000000-0000-0000-0000-00000000aaaa","workspace_type":"private","workspace_role":"owner","workspace_membership_exists":false,"blog_ip_check":false,"blog_collab":false,"subscription_downgrades_at":null,"last_daily_credits_awarded_at":"2026-08-31T03:26:11.951412+00:00","has_flex_unlim":true,"hide_reduced_nano_banana_2_concurrent":true,"unlim_battery":null,"blocked_at":null,"suspended_at":null,"email":"user@example.com","business_email":null,"block_reason":"other","appeal_applied_at":null,"plan_version":40,"show_generation_activity_notice":false,"verified_business_email":null,"is_test_user":false,"mcp_trial_ends_at":null,"trial_status":null,"consents":{"enabled":false,"terms_of_use":{"required_version":2,"accepted_version":2,"is_accepted":true},"biometric":{"required_version":1,"accepted_version":0,"is_accepted":false}},"cobalt":true}"#;

  #[test]
  fn user_response_parses() {
    let response: UserDataResponse = serde_json::from_str(USER_RESPONSE).unwrap();
    assert_eq!(response.id.as_str(), "user_TESTUSER0000000000000000000");
    assert_eq!(response.email.as_deref(), Some("user@example.com"));
    assert_eq!(response.plan_type, Some(PlanType::Plus));
    assert_eq!(response.billing_period, Some(BillingPeriod::Monthly));
    assert_eq!(response.subscription_credits, Some(1200.0));
    assert_eq!(response.total_plan_credits, Some(1200.0));
    assert!(response.has_unlim);
    assert_eq!(response.workspace_id.as_ref().unwrap().as_str(), "00000000-0000-0000-0000-00000000aaaa");
    assert_eq!(response.workspace_role.as_deref(), Some("owner"));
    assert!(response.is_account_active());

    // Untyped fields are still reachable.
    assert_eq!(response.extra.get("cohort").and_then(|v| v.as_str()), Some("tier_1"));
    assert!(response.extra.contains_key("consents"));
  }

  #[test]
  fn blocked_account_is_inactive() {
    let json = USER_RESPONSE.replace("\"blocked_at\":null", "\"blocked_at\":\"2026-08-31T00:00:00+00:00\"");
    let response: UserDataResponse = serde_json::from_str(&json).unwrap();
    assert!(!response.is_account_active());
  }

  #[test]
  fn unknown_plan_parses_as_other() {
    let json = USER_RESPONSE.replace("\"plan_type\":\"plus\"", "\"plan_type\":\"galactic\"");
    let response: UserDataResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(response.plan_type, Some(PlanType::Other("galactic".to_string())));
  }

  // ── Live (ignored: needs a real session) ──

  /// Drives the binding off the desktop app's saved Higgsfield login
  /// (`~/Artcraft/artcraftx/credentials/higgsfield_cookies.toml`): mints a
  /// Clerk token from the stored cookies, calls `/fnf/user`, and prints
  /// everything that came back.
  #[tokio::test]
  #[ignore]
  async fn live_user_data_from_app_credential() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("minting a session token failed: {err}"))?;
    println!("Minted bearer token ({} chars); session {:?}", auth.bearer_token.len(), session.current_token().await.map(|t| t.session_id().to_string()));

    let response = user_data(UserDataArgs {
      request: UserDataRequest,
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    println!("\n===== /fnf/user (typed) =====\n{:#?}", response);
    println!("\n===== /fnf/user (extra, untyped) =====");
    for (key, value) in &response.extra {
      println!("{key} = {value}");
    }

    assert!(response.email.is_some(), "expected an email on the account");
    assert!(response.workspace_id.is_some(), "expected a workspace id");
    Ok(())
  }

  #[tokio::test]
  #[ignore]
  async fn live_user_data() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_auth;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let auth = load_higgsfield_test_auth().await?;
    let response = user_data(UserDataArgs {
      request: UserDataRequest,
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    println!("User {} plan={:?} credits={:?}", response.id, response.plan_type, response.subscription_credits);
    assert!(response.email.is_some());
    Ok(())
  }
}
