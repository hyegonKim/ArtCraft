/// Centralize all of the Redis key naming and dereferencing.
pub struct RedisKeys;

impl RedisKeys {

  // =============== FAKEYOU / TTS / W2L ===============

  /// This is a counter incremented with TTS inference.
  /// We can use a job to read MySql and fix counts if they get out of sync.
  /// These should be long-lived keys.
  pub fn tts_model_usage_count(model_token: &str) -> String {
    format!("ttsUseCount:{}", model_token)
  }

  /// This is a counter incremented with web VC inference.
  /// We can use a job to read MySql and fix counts if they get out of sync.
  /// These should be long-lived keys.
  pub fn web_vc_model_usage_count(model_token: &str) -> String {
    format!("vcUseCount:{}", model_token)
  }

  /// This is a counter incremented with TTS inference.
  /// We can use a job to read MySql and fix counts if they get out of sync.
  /// These should be long-lived keys.
  pub fn w2l_template_usage_count(template_token: &str) -> String {
    format!("w2lUseCount:{}", template_token)
  }

  /// We write extra in-progress status information to keys.
  /// These keys should have a TTL.
  pub fn tts_download_extra_status_info(inference_job_token: &str) -> String {
    format!("ttsDownloadExtraStatus:{}", inference_job_token)
  }

  /// We write extra in-progress status information to keys.
  /// These keys should have a TTL.
  pub fn generic_download_extra_status_info(inference_job_token: &str) -> String {
    format!("genericDownloadExtraStatus:{}", inference_job_token)
  }

  /// We write extra in-progress status information to keys.
  /// These keys should have a TTL.
  pub fn generic_inference_extra_status_info(inference_job_token: &str) -> String {
    format!("genericInferenceExtraStatus:{}", inference_job_token)
  }

  /// Keep alive for certain generic inference jobs
  pub fn generic_inference_keepalive(inference_job_token: &str) -> String {
    format!("genericInferenceKeepAlive:{}", inference_job_token)
  }

  /// We write extra in-progress status information to keys.
  /// These keys should have a TTL.
  pub fn tts_inference_extra_status_info(inference_job_token: &str) -> String {
    format!("ttsInferenceExtraStatus:{}", inference_job_token)
  }

  /// We write extra in-progress status information to keys.
  /// These keys should have a TTL.
  pub fn w2l_download_extra_status_info(inference_job_token: &str) -> String {
    format!("w2lDownloadExtraStatus:{}", inference_job_token)
  }

  /// We write extra in-progress status information to keys.
  /// These keys should have a TTL.
  pub fn w2l_inference_extra_status_info(inference_job_token: &str) -> String {
    format!("w2lInferenceExtraStatus:{}", inference_job_token)
  }

  // =============== OBS / TWITCH ===============

  /// This is a PubSub topic.
  /// We publish when new viewers arrive and as a "keep-alive ping".
  pub fn obs_active_sessions_topic() -> &'static str {
    "obsActiveSessionsTopic"
  }

  /// This is a key that denotes a user is actively on the OBS gateway.
  /// These keys have a short expiry and OBS gateway must continually bump them.
  /// If a key goes away, the thread monitoring that user should exit.
  pub fn obs_active_session_keepalive(twitch_user_id: &str) -> String {
    format!("obsActiveSessionKeepalive:{}", twitch_user_id)
  }

  /// This is a key that acts as a lock guaranteeing that there is at most *only one*
  /// Twitch PubSub subscriber per Twitch streamer, no matter how many browsers are open.
  /// Only one thread should be PubSub subscribed to any given user at a time.
  /// These keys should have a TTL so that they naturally expire without maintenance.
  pub fn twitch_pubsub_lease(twitch_user_id: &str) -> String {
    format!("twitchPubsubLease:{}", twitch_user_id)
  }

  // TODO/FIXME: This appears to be unused.
  /// This is a PubSub topic.
  /// These are "unenriched" Twitch events from the PubSub (and eventually IRC)
  /// subscribers. Downstream listeners will enrich these for user-facing functionality.
  pub fn twitch_events_topic(twitch_user_id: &str) -> String {
    format!("twitchEventsTopic:{}", twitch_user_id)
  }

  /// This is a lazy first approximation of the worker -> OBS messaging.
  /// We push new TTS job tokens into a Redis list (with long expiry), and OBS
  /// will dequeue the events in order.
  #[deprecated(note="Use twitch_tts_job_topic instead (PubSub instead of a Redis list)")]
  pub fn twitch_tts_job_queue(twitch_user_id: &str) -> String {
    format!("twitchTtsJobs:{}", twitch_user_id)
  }

  /// This is a PubSub topic.
  /// These are TTS job tokens pushed to a Redis PubSub topic (per-user!). This should
  /// have better usability characteristics than a mutable Redis list that sends different
  /// browsers different states (race conditions from first principles!)
  pub fn twitch_tts_job_topic(twitch_user_id: &str) -> String {
    format!("twitchTtsJobTopic:{}", twitch_user_id)
  }
}
