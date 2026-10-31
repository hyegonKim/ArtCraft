use higgsfield_client::session::higgsfield_session::HiggsfieldSession;

/// The first-party (cookie-session) Higgsfield client used by the router.
///
/// Drives the user's own logged-in higgsfield.ai account through a
/// [`HiggsfieldSession`], which mints and refreshes the short-lived Clerk
/// bearer token on its own and replays the captured cookies under the
/// User-Agent that earned them. The router enqueues jobs (and uploads
/// reference media first); the app polls job status itself.
pub struct RouterHiggsfieldClient {
  pub(crate) session: HiggsfieldSession,
}

impl RouterHiggsfieldClient {
  pub fn new(session: HiggsfieldSession) -> Self {
    Self { session }
  }

  pub fn session(&self) -> &HiggsfieldSession {
    &self.session
  }
}
