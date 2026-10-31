//! This matches the definition in storyteller-web and lets us send
//! downtime messages to users of the website.

use std::sync::Arc;
use std::time::Duration;

use actix_web::{HttpResponse, web};
use serde_derive::Serialize;

use crate::endpoints::status_alert_handler::StatusAlertCategory::DownForMaintenance;
use crate::server_state::ServerState;

/// How often the client should poll
const REFRESH_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Serialize)]
pub struct StatusAlertResponse {
  pub success: bool,

  /// If there's an alert, this will be set.
  /// The sub keys are optional, but at least one of them will be set.
  /// i.e. we can have an alert with no message or no predefined category.
  pub maybe_alert: Option<StatusAlertInfo>,

  /// Tell the frontend client how fast to refresh their view of this list.
  /// During an attack, we may want this to go extremely slow.
  pub refresh_interval_millis: u64,
}

#[derive(Serialize)]
pub struct StatusAlertInfo {
  /// If an alert is set, this might be a key for a common i18n message to show.
  pub maybe_category: Option<StatusAlertCategory>,

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

#[derive(Serialize, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum StatusAlertCategory {
  /// System is down for maintenance
  DownForMaintenance,
}

pub async fn status_alert_handler(
  server_state: web::Data<Arc<ServerState>>
) -> HttpResponse {
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
    (category, message) => Some(StatusAlertInfo {
      maybe_category: category,
      maybe_message: message,
    }),
  };

  let response = StatusAlertResponse {
    success: true,
    maybe_alert,
    refresh_interval_millis: REFRESH_INTERVAL.as_millis() as u64,
  };

  match serde_json::to_string(&response) {
    Ok(body) => HttpResponse::Ok()
        .content_type("application/json")
        .body(body),
    Err(_err) => HttpResponse::Ok()
        .content_type("application/json")
        .body("{\"success\": false}"),
  }
}

fn category_to_enum(category: &str) -> Option<StatusAlertCategory> {
  let key = category.trim();
  match key {
    "down_for_maintenance" => Some(DownForMaintenance),
    _ => None,
  }
}
