use datadome_errors::datadome_error::DataDomeError;

/// The counter-measure for a DataDome rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataDomeMitigation {
  /// A challenge a browser can pass. Send the user through the site's login
  /// window to earn a fresh `datadome` cookie, then replay it with the
  /// matching `x-datadome-clientid` header and User-Agent
  /// (see `client_id` and `fingerprint`).
  ReauthenticateInBrowser,

  /// DataDome has banned this client identity. A re-login from the same
  /// IP / browser profile will be banned too; surface it and stop.
  GiveUp,
}

impl DataDomeMitigation {
  /// Whether the session needs a human in a browser before continuing.
  pub fn needs_browser(&self) -> bool {
    matches!(self, Self::ReauthenticateInBrowser)
  }
}

/// The mitigation for an error. DataDome never asks us to simply retry:
/// every rejection is about the client identity.
pub fn mitigation_for(error: &DataDomeError) -> DataDomeMitigation {
  match error {
    DataDomeError::CaptchaChallenge { .. } | DataDomeError::Interstitial { .. } => DataDomeMitigation::ReauthenticateInBrowser,
    DataDomeError::Blocked { .. } => DataDomeMitigation::GiveUp,
    // We know DataDome rejected us but not how; a fresh browser session is
    // the only move that can help.
    DataDomeError::Unclassified { .. } => DataDomeMitigation::ReauthenticateInBrowser,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn challenges_need_a_browser() {
    let error = DataDomeError::CaptchaChallenge { challenge_url: "https://geo.captcha-delivery.com/captcha/".into() };
    assert_eq!(mitigation_for(&error), DataDomeMitigation::ReauthenticateInBrowser);
    assert!(mitigation_for(&error).needs_browser());
  }

  #[test]
  fn hard_blocks_give_up() {
    let error = DataDomeError::Blocked { maybe_block_url: None };
    assert_eq!(mitigation_for(&error), DataDomeMitigation::GiveUp);
  }
}
