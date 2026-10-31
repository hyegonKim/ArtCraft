use actix_web::HttpRequest;

/// Allows us to purge session caches that might keep stale premium plan/billing data around
#[cfg_attr(test, mockall::automock)]
pub trait InternalSessionCachePurge {
  /// Best effort attempt to delete
  /// Fails silently.
  fn best_effort_purge_session_cache(&self, http_request: &HttpRequest);
}
