use serde_derive::{Deserialize, Serialize};

use crate::tokens::non_unique::debug_logs_event_token::DebugLogEventToken;

use crate::api_defs::moderation::debug_logs::debug_log_entry::ModerationDebugLogEntry;

pub const MODERATION_LIST_DEBUG_LOGS_FOR_TOKEN_PATH: &str = "/v1/moderation/debug_logs/list/{token}";

// ── Path params ──

#[derive(Deserialize)]
pub struct ModerationListDebugLogsForTokenPathInfo {
  pub token: DebugLogEventToken,
}

// ── Query params ──

#[derive(Deserialize)]
pub struct ModerationListDebugLogsForTokenQueryParams {
  pub limit: Option<u32>,
}

// ── Response ──

#[derive(Serialize)]
pub struct ModerationListDebugLogsForTokenSuccessResponse {
  pub success: bool,
  pub debug_logs: Vec<ModerationDebugLogEntry>,
}
