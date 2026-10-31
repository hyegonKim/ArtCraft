use crate::state::server_state::ServerState;
use primitives::truncate_str::truncate_str;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct AppStateServerInfo {
  /// The GIT SHA of the server build.
  pub build_sha: String,

  /// The GIT SHA of the server build (short).
  pub build_sha_short: String,

  /// The hostname of the server (that returned this response).
  pub hostname: String,
}

pub fn get_server_info(
  server_state: &ServerState,
) -> AppStateServerInfo {
  let short_sha = truncate_str(&server_state.server_info.build_sha, 8);
  AppStateServerInfo {
    build_sha: server_state.server_info.build_sha.clone(),
    build_sha_short: short_sha.to_string(),
    hostname: server_state.hostname.clone(),
  }
}
