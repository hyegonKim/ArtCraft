use crate::server_state::ServerState;
use actix_web::web;
use actix_web::web::Json;
use serde_derive::Serialize;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

/// How often the client should poll
const REFRESH_INTERVAL: Duration = Duration::from_secs(60);
#[derive(Serialize)]
pub struct AppStateResponse {
  pub success: bool,
  pub refresh_interval_millis: u128,
  pub server_info: AppStateServerInfo,
  pub maybe_alert: Option<AppStateStatusAlertInfo>,
  pub locale: AppStateUserLocale,
  pub is_logged_in: bool,
  pub is_banned: bool,
  pub maybe_user_info: Option<()>, // Option<AppStateUserInfo>,
  pub permissions: AppStatePermissions,
  pub maybe_premium: Option<()>, // Option<AppStatePremiumInfo>,
}

#[derive(Serialize)]
pub struct AppStateServerInfo {
  pub build_sha: String,
  pub build_sha_short: String,
  pub hostname: String,
}

#[derive(Serialize)]
pub struct AppStateStatusAlertInfo {
  pub maybe_category: Option<AppStateStatusAlertCategory>,
  pub maybe_message: Option<String>,
}

#[derive(Serialize)]
pub enum AppStateStatusAlertCategory {
  DownForMaintenance,
}

#[derive(Serialize)]
pub struct AppStateUserLocale {
  pub full_language_tags: Vec<String>,
  pub language_codes: Vec<String>,
}

#[derive(Serialize)]
pub struct AppStatePermissions {
  pub is_moderator: bool,
  pub feature_flags: BTreeSet<String>, // pub feature_flags: BTreeSet<UserFeatureFlag>,
  pub legacy_permission_flags: AppStateLegacyPermissionFlags,
}

#[derive(Serialize)]
pub struct AppStateLegacyPermissionFlags {
  // Usage permissions:
  pub can_use_tts: bool,
  pub can_use_w2l: bool,
  pub can_delete_own_tts_results: bool,
  pub can_delete_own_w2l_results: bool,
  pub can_delete_own_account: bool,
  // Contribution permissions:
  pub can_upload_tts_models: bool,
  pub can_upload_w2l_templates: bool,
  pub can_delete_own_tts_models: bool,
  pub can_delete_own_w2l_templates: bool,
  // Moderation permissions:
  pub can_approve_w2l_templates: bool,
  pub can_edit_other_users_profiles: bool,
  pub can_edit_other_users_tts_models: bool,
  pub can_edit_other_users_w2l_templates: bool,
  pub can_delete_other_users_tts_models: bool,
  pub can_delete_other_users_tts_results: bool,
  pub can_delete_other_users_w2l_templates: bool,
  pub can_delete_other_users_w2l_results: bool,
  pub can_ban_users: bool,
  pub can_delete_users: bool,
}

pub async fn dummy_app_state_handler(
  server_state: web::Data<Arc<ServerState>>
) -> Json<AppStateResponse> {

  let maybe_category = server_state
      .flags
      .maybe_status_alert_category
      .as_deref()
      .map(|category| category_to_enum(&category))
      .flatten();

  let maybe_message = server_state
      .flags
      .maybe_status_alert_custom_message
      .as_deref()
      .map(|message| message.trim().to_string());

  let maybe_alert = match (maybe_category, maybe_message) {
    (None, None) => None,
    (category, message) => Some(AppStateStatusAlertInfo {
      maybe_category: category,
      maybe_message: message,
    }),
  };

  Json(AppStateResponse {
    success: true,
    refresh_interval_millis: REFRESH_INTERVAL.as_millis(),
    maybe_alert,
    server_info: AppStateServerInfo {
      build_sha: "aabbcc".to_string(),
      build_sha_short: "aabbcc".to_string(),
      hostname: "hostname".to_string(),
    },
    locale: AppStateUserLocale {
      full_language_tags: vec!["en-US".to_string()],
      language_codes: vec!["en".to_string()],
    },
    is_logged_in: false,
    is_banned: false,
    maybe_user_info: None,
    maybe_premium: None,
    permissions: AppStatePermissions {
      is_moderator: false,
      feature_flags: BTreeSet::new(),
      legacy_permission_flags: AppStateLegacyPermissionFlags {
        can_use_tts: false,
        can_use_w2l: false,
        can_delete_own_tts_results: false,
        can_delete_own_w2l_results: false,
        can_delete_own_account: false,
        can_upload_tts_models: false,
        can_upload_w2l_templates: false,
        can_delete_own_tts_models: false,
        can_delete_own_w2l_templates: false,
        can_approve_w2l_templates: false,
        can_edit_other_users_profiles: false,
        can_edit_other_users_tts_models: false,
        can_edit_other_users_w2l_templates: false,
        can_delete_other_users_tts_models: false,
        can_delete_other_users_tts_results: false,
        can_delete_other_users_w2l_templates: false,
        can_delete_other_users_w2l_results: false,
        can_ban_users: false,
        can_delete_users: false,
      },
    },
  })
}

fn category_to_enum(category: &str) -> Option<AppStateStatusAlertCategory> {
  let key = category.trim();
  match key {
    "down_for_maintenance" => Some(AppStateStatusAlertCategory::DownForMaintenance),
    _ => None,
  }
}
