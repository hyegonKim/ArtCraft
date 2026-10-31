//! Test-only loader for the desktop app's saved Higgsfield credential file
//! (`~/Artcraft/artcraftx/credentials/higgsfield_cookies.toml`), written by
//! the login window. Lets live tests run straight off a real app login
//! without copying cookies around.
//!
//! Override the path with `HIGGSFIELD_CREDENTIAL_TOML=/path/to/file.toml`.

use crate::credentials::higgsfield_cookies::HiggsfieldCookies;
use crate::session::higgsfield_session::HiggsfieldSession;
use errors::AnyhowResult;
use serde::Deserialize;
use std::fs::read_to_string;
use std::path::PathBuf;

const PATH_ENV_VAR: &str = "HIGGSFIELD_CREDENTIAL_TOML";
const DEFAULT_RELATIVE_PATH: &str = "Artcraft/artcraftx/credentials/higgsfield_cookies.toml";

/// The subset of the app's credential TOML we need. (Mirrors
/// `crates/artcraftx/src/credentials/credential_toml.rs`, which this crate
/// can't depend on.)
#[derive(Deserialize)]
struct CredentialToml {
  service: String,

  #[serde(default)]
  user_info: Option<UserInfoToml>,

  cookie: CookieSectionToml,
}

#[derive(Deserialize)]
struct UserInfoToml {
  #[serde(default)]
  email: Option<String>,
}

#[derive(Deserialize)]
struct CookieSectionToml {
  #[serde(default)]
  cookies: Vec<CookieToml>,
}

#[derive(Deserialize)]
struct CookieToml {
  name: String,
  value: String,
}

/// Where the credential file lives.
pub fn higgsfield_credential_toml_path() -> AnyhowResult<PathBuf> {
  if let Ok(path) = std::env::var(PATH_ENV_VAR) {
    return Ok(PathBuf::from(path));
  }
  let home = std::env::var("HOME").map_err(|_| anyhow::anyhow!("HOME is not set"))?;
  Ok(PathBuf::from(home).join(DEFAULT_RELATIVE_PATH))
}

/// The cookies from the app's credential file, as one cookie header (every
/// cookie regardless of host — the gateway and Clerk both live under
/// higgsfield.ai, and the session needs Clerk's `__client`).
pub fn load_higgsfield_cookies_from_app_credential() -> AnyhowResult<HiggsfieldCookies> {
  let path = higgsfield_credential_toml_path()?;
  let raw = read_to_string(&path)
      .map_err(|err| anyhow::anyhow!("Could not read Higgsfield credential {:?}: {} (log in via the app first)", path, err))?;
  let parsed: CredentialToml = toml::from_str(&raw)
      .map_err(|err| anyhow::anyhow!("Could not parse Higgsfield credential {:?}: {}", path, err))?;

  if parsed.service != "higgsfield_cookies" {
    anyhow::bail!("{:?} is a {:?} credential, not higgsfield_cookies", path, parsed.service);
  }

  let header = parsed.cookie.cookies.iter()
      .map(|cookie| format!("{}={}", cookie.name, cookie.value))
      .collect::<Vec<_>>()
      .join("; ");

  println!(
    "Loaded Higgsfield credential {:?}: {} cookies, email={:?}",
    path,
    parsed.cookie.cookies.len(),
    parsed.user_info.and_then(|info| info.email),
  );

  Ok(HiggsfieldCookies::from_cookie_header(header))
}

/// A session backed by the app's credential file.
pub fn load_higgsfield_session_from_app_credential() -> AnyhowResult<HiggsfieldSession> {
  Ok(HiggsfieldSession::from_cookies(load_higgsfield_cookies_from_app_credential()?))
}
