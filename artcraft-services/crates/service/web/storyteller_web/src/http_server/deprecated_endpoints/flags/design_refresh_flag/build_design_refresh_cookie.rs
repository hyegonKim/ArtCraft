use actix_web::cookie::{Cookie, CookieBuilder};

use crate::state::server_state::ServerState;

const REFRESH_COOKIE_NAME : &str = "refresh";

pub fn build_design_refresh_cookie(server_state: &ServerState, enable: bool) -> CookieBuilder {
  let value = if enable { "true" } else { "false " };

  Cookie::build(REFRESH_COOKIE_NAME, value)
      .domain(&server_state.env_config.cookie_domain)
      .secure(server_state.env_config.cookie_secure) // HTTPS-only
      .path("/")
      .http_only(false) // This is meant to be exposed to Javascript!
      .permanent()
}
