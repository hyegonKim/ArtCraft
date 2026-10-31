use actix_web::HttpRequest;
use log::info;

use redis_common::redis_cache_keys::RedisCacheKeys;

use crate::state::server_state::ServerState;

pub fn try_delete_session_cache(
  http_request: &HttpRequest,
  server_state: &ServerState,
) {
  // TODO: Clear Redis cache of sessions
  //  Unfortunately we don't yet have an index of user_token => session_tokens[] outside the DB.
  //  For now, a hacky solution is just to delete the cache under the current user.
  //  This makes sense for non-mods and should solve 95% of cases.
  let session_token = match server_state.session_checker.forgiving_get_session_token(&http_request) {
    None => return,
    Some(session_token) => session_token,
  };

  if let Ok(mut redis_ttl_cache) = server_state.redis_ttl_cache.get_connection() {
    info!("Delete session cache for user.");
    let keys = vec![
      RedisCacheKeys::session_record_user(&session_token),
      RedisCacheKeys::session_record_light(&session_token),
    ];
    for key in keys.iter() {
      let _r = redis_ttl_cache.delete_from_cache(key).ok();
    }
  }
}
