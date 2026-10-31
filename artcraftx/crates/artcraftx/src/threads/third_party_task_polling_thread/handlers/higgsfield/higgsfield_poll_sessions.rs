use std::collections::HashMap;

use higgsfield_client::session::higgsfield_session::HiggsfieldSession;
use log::{info, warn};

use crate::credentials::auth_credential::AuthCredential;
use crate::services::higgsfield::higgsfield_session_from_credential::higgsfield_session_from_credential;
use crate::state::data_dir::app_data_root::AppDataRoot;
use core_types::enums::generation_source::GenerationSource;

/// One live [`HiggsfieldSession`] per stored Higgsfield credential, kept
/// across polling iterations so the short-lived Clerk bearer token is reused
/// rather than re-minted every couple of seconds. Sessions follow the
/// credential files: a re-login (new cookies) replaces the session, a deleted
/// credential drops it.
#[derive(Default)]
pub struct HiggsfieldPollSessions {
  sessions: HashMap<String, SessionEntry>,
}

struct SessionEntry {
  cookie_header: String,
  session: HiggsfieldSession,
}

impl HiggsfieldPollSessions {
  pub fn new() -> Self {
    Self::default()
  }

  /// The sessions for every Higgsfield credential currently on disk, in
  /// credential order. Empty when the user has no Higgsfield account.
  pub fn refresh(&mut self, app_data_root: &AppDataRoot) -> Vec<(String, HiggsfieldSession)> {
    let credentials = match app_data_root.credentials_dir().load_credentials() {
      Ok(credentials) => credentials,
      Err(err) => {
        warn!("[HiggsfieldPolling] Could not load credentials: {}", err);
        return self.sessions.iter().map(|(id, entry)| (id.clone(), entry.session.clone())).collect();
      }
    };

    let higgsfield_credentials: Vec<AuthCredential> = credentials.into_iter()
        .filter(|credential| credential.service == GenerationSource::HiggsfieldCookies)
        .collect();

    let live_ids: Vec<String> = higgsfield_credentials.iter().map(|c| c.id.as_str().to_string()).collect();
    self.sessions.retain(|id, _| live_ids.contains(id));

    let mut result = Vec::with_capacity(higgsfield_credentials.len());
    for credential in &higgsfield_credentials {
      let id = credential.id.as_str().to_string();
      // A session Higgsfield already rejected stays untouched until the user
      // logs in again (which rewrites the cookies and clears the mark).
      if credential.needs_relogin() {
        if self.sessions.remove(&id).is_some() {
          warn!("[HiggsfieldPolling] Credential {} needs a re-login; not polling with it", id);
        }
        continue;
      }
      let cookie_header = credential.cookies().map(|cookie| cookie.cookie_header()).unwrap_or_default();
      let is_current = self.sessions.get(&id).is_some_and(|entry| entry.cookie_header == cookie_header);
      if !is_current {
        match higgsfield_session_from_credential(credential) {
          Ok(session) => {
            info!("[HiggsfieldPolling] Opened a session for credential {}", id);
            self.sessions.insert(id.clone(), SessionEntry { cookie_header, session });
          }
          Err(err) => {
            warn!("[HiggsfieldPolling] Credential {} can't open a Higgsfield session: {}", id, err);
            self.sessions.remove(&id);
            continue;
          }
        }
      }
      if let Some(entry) = self.sessions.get(&id) {
        result.push((id, entry.session.clone()));
      }
    }
    result
  }
}
