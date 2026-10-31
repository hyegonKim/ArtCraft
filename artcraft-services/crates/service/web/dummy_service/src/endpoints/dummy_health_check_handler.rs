use actix_web::HttpResponse;
use chrono::{NaiveDateTime, Utc};
use serde_derive::Serialize;

#[derive(Serialize)]
pub struct HealthCheckResponse {
  pub success: bool,
  pub is_healthy: bool,
  pub last_db_time: Option<NaiveDateTime>,
  pub healthy_check_consecutive_count: Option<u64>,
  pub unhealthy_check_consecutive_count: Option<u64>,
  pub server_build_sha: String,
  pub server_hostname: String,
}

pub async fn dummy_health_check_handler() -> HttpResponse {
  let response = HealthCheckResponse {
    success: true,
    is_healthy: true,
    last_db_time: Some(Utc::now().naive_utc()),
    healthy_check_consecutive_count: Some(1_234),
    unhealthy_check_consecutive_count: None,
    server_build_sha: "aabbcc".to_string(),
    server_hostname: "hostname".to_string(),
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
