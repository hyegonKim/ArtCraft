use std::time::Duration;

use actix_web::HttpResponse;
use chrono::{NaiveDateTime, Utc};
use serde_derive::Serialize;

/// How often the client should poll
const REFRESH_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Serialize)]
pub struct GetUnifiedQueueStatsSuccessResponse {
  pub success: bool,
  pub cache_time: NaiveDateTime,

  /// Tell the frontend client how fast to refresh their view of this list.
  /// During an attack, we may want this to go extremely slow.
  pub refresh_interval_millis: u64,

  pub inference: ModernInferenceQueueStats,
  pub legacy_tts: LegacyQueueDetails,
}

#[derive(Serialize)]
pub struct LegacyQueueDetails {
  pub pending_job_count: u64,
}

#[derive(Serialize)]
pub struct ModernInferenceQueueStats {
  pub total_pending_job_count: u64,

  #[deprecated(note="the frontend uses this field, but we should switch to total_pending_job_count")]
  pub pending_job_count: u64,

  pub by_queue: ByQueueStats,
}

#[derive(Serialize)]
pub struct ByQueueStats {
  pub pending_face_animation_jobs: u64,
  pub pending_rvc_jobs: u64,
  pub pending_svc_jobs: u64,
  pub pending_tacotron2_jobs: u64,
  pub pending_voice_designer: u64,
  pub pending_stable_diffusion: u64,
}

pub async fn dummy_queue_stats_handler() -> HttpResponse {
  let response = GetUnifiedQueueStatsSuccessResponse {
    success: true,
    cache_time: Utc::now().naive_utc(),
    refresh_interval_millis: REFRESH_INTERVAL.as_millis() as u64,
    inference: ModernInferenceQueueStats {
      total_pending_job_count: 12_345,
      pending_job_count: 12_345,
      by_queue: ByQueueStats {
        pending_face_animation_jobs: 12_345,
        pending_rvc_jobs: 12_345,
        pending_svc_jobs: 12_345,
        pending_tacotron2_jobs: 12_345,
        pending_voice_designer: 12_345,
        pending_stable_diffusion: 12_345,
      }
    },
    legacy_tts: LegacyQueueDetails {
      pending_job_count: 12_345,
    },
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
