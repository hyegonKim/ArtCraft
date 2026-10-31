//! Test-only loader for a browser-captured Higgsfield session kept off to
//! the side in the (gitignored) `external/credentials/higgsfield` directory.
//!
//! Files (all plain text, whitespace trimmed):
//!
//! - `cookies.txt` — optional. The `cookie` header for higgsfield.ai,
//!   INCLUDING the `__client` cookie from `clerk.higgsfield.ai` (in DevTools:
//!   Application → Cookies → both hosts). When absent, the desktop app's
//!   saved login (`~/Artcraft/artcraftx/credentials/higgsfield_cookies.toml`)
//!   is used instead — so logging in through the app is enough.
//! - `bearer.txt` — optional. A captured `authorization: Bearer …` JWT; when
//!   present it's used as-is (it expires after ~60s, so re-capture right
//!   before running). Otherwise a token is minted from the cookies.
//! - `datadome_client_id.txt` — optional. The `x-datadome-clientid` header.
//! - `job_id.txt` — optional. A single job id for the status live tests.
//! - `job_ids.txt` — optional. Job ids for the status live tests, one per
//!   line (`#` comments allowed). Real ids stay out of the repo.

use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::credentials::higgsfield_cookies::HiggsfieldCookies;
use crate::session::higgsfield_session::HiggsfieldSession;
use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
use crate::types::ids::JobId;
use errors::AnyhowResult;
use std::fs::read_to_string;
use std::path::Path;

/// Repo-root-relative directory holding the captured secrets. Resolved from
/// this crate's manifest dir so tests work regardless of the shell's working
/// directory.
const SECRETS_DIR: &str = concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/../../../external/credentials/higgsfield",
);

/// The captured cookies.
pub fn load_higgsfield_test_cookies() -> AnyhowResult<HiggsfieldCookies> {
  Ok(HiggsfieldCookies::from_cookie_header(read_required("cookies.txt")?))
}

/// A session that mints its own tokens: from `cookies.txt` when present,
/// else from the desktop app's saved Higgsfield login (see
/// `higgsfield_credential_toml`), so a plain app login is enough to run
/// every live test.
pub fn load_higgsfield_test_session() -> AnyhowResult<HiggsfieldSession> {
  if read_optional("cookies.txt")?.is_none() {
    return load_higgsfield_session_from_app_credential();
  }
  let mut session = HiggsfieldSession::from_cookies(load_higgsfield_test_cookies()?);
  if let Some(datadome_client_id) = read_optional("datadome_client_id.txt")? {
    session = session.with_datadome_client_id(datadome_client_id);
  }
  Ok(session)
}

/// A bearer auth for the raw endpoint bindings: a captured `bearer.txt` if
/// present, else minted by [`load_higgsfield_test_session`].
pub async fn load_higgsfield_test_auth() -> AnyhowResult<HiggsfieldAuth> {
  if read_optional("bearer.txt")?.is_none() {
    let session = load_higgsfield_test_session()?;
    return session.auth().await.map_err(|err| anyhow::anyhow!("Could not mint a Higgsfield session token: {err}"));
  }

  let bearer = read_required("bearer.txt")?;
  let mut auth = HiggsfieldAuth::new(bearer);
  if let Some(datadome_client_id) = read_optional("datadome_client_id.txt")? {
    auth = auth.with_datadome_client_id(datadome_client_id);
  }
  Ok(auth)
}

/// A job id to poll in the status live tests: the first of `job_ids.txt`,
/// else `job_id.txt`.
pub fn load_higgsfield_test_job_id() -> AnyhowResult<JobId> {
  if let Some(first) = load_higgsfield_test_job_ids()?.into_iter().next() {
    return Ok(first);
  }
  Ok(JobId::new(read_required("job_id.txt")?))
}

/// Every job id listed in `job_ids.txt` (falling back to `job_id.txt`).
/// Errors if neither file has any.
pub fn load_higgsfield_test_job_ids() -> AnyhowResult<Vec<JobId>> {
  let mut ids: Vec<JobId> = read_optional("job_ids.txt")?
      .unwrap_or_default()
      .lines()
      .map(|line| line.split('#').next().unwrap_or("").trim())
      .filter(|line| !line.is_empty())
      .map(JobId::new)
      .collect();

  if ids.is_empty() {
    if let Some(single) = read_optional("job_id.txt")? {
      ids.push(JobId::new(single));
    }
  }

  if ids.is_empty() {
    anyhow::bail!("No Higgsfield test job ids: add {}/job_ids.txt (one id per line) from a job this account ran", SECRETS_DIR);
  }
  Ok(ids)
}

fn read_required(file_name: &str) -> AnyhowResult<String> {
  read_optional(file_name)?.ok_or_else(|| {
    anyhow::anyhow!("Missing Higgsfield test secret {}/{} (capture it from a logged-in browser session)", SECRETS_DIR, file_name)
  })
}

fn read_optional(file_name: &str) -> AnyhowResult<Option<String>> {
  let path = Path::new(SECRETS_DIR).join(file_name);
  if !path.exists() {
    return Ok(None);
  }
  let raw = read_to_string(&path)
      .map_err(|err| anyhow::anyhow!("Could not read Higgsfield test secret {:?}: {}", path, err))?;
  let trimmed = raw.trim().to_string();
  Ok((!trimmed.is_empty()).then_some(trimmed))
}
