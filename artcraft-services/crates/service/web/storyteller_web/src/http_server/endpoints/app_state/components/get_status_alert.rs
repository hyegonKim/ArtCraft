use crate::state::server_state::ServerState;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema, Copy, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AppStateStatusAlertCategory {
  /// System is down for maintenance
  DownForMaintenance,
}

#[derive(Serialize, ToSchema)]
pub struct AppStateStatusAlertInfo {
  /// If an alert is set, this might be a key for a common i18n message to show.
  pub maybe_category: Option<AppStateStatusAlertCategory>,

  /// We can set an optional message to show to the frontend.
  ///
  /// This field may or may not be set. If it's set, this is custom text that should
  /// be displayed to the user. For example, we might tell the user when we'll be
  /// back online, etc. Both this field and `maybe_category` are optional.
  ///
  /// If `maybe_category` is present, it should be a key for a predefined and
  /// internationalized (i18n) message stored on the frontend.
  ///
  /// Either or both of these fields may be set.
  pub maybe_message: Option<String>,
}

pub fn get_status_alert(
  server_state: &ServerState
) -> Option<AppStateStatusAlertInfo> {

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

  match (maybe_category, maybe_message) {
    (None, None) => None,
    (category, message) => Some(AppStateStatusAlertInfo {
      maybe_category: category,
      maybe_message: message,
    }),
  }
}

fn category_to_enum(category: &str) -> Option<AppStateStatusAlertCategory> {
  let key = category.trim();
  match key {
    "down_for_maintenance" => Some(AppStateStatusAlertCategory::DownForMaintenance),
    _ => None,
  }
}
