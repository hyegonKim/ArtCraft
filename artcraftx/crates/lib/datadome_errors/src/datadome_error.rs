use log::Level;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// A response produced by DataDome's bot protection instead of the site.
///
/// All variants mean "this client was not trusted"; they differ in whether
/// a browser could get it trusted again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataDomeError {
  /// A CAPTCHA challenge (`/captcha/`). A real browser can solve it and earn
  /// a fresh `datadome` cookie; the replaying client must then present the
  /// same cookie AND fingerprint (User-Agent, `x-datadome-clientid`).
  CaptchaChallenge {
    /// The challenge page URL DataDome returned.
    challenge_url: String,
  },

  /// A device-check interstitial (`/interstitial/`): a JavaScript proof
  /// rather than a visible puzzle. Same remedy as a CAPTCHA.
  Interstitial {
    challenge_url: String,
  },

  /// A hard block (`t=bv`): DataDome has decided this client is a bot and
  /// won't offer a challenge. Only a new identity (IP / browser session)
  /// clears it.
  Blocked {
    /// The block page URL, if one was given.
    maybe_block_url: Option<String>,
  },

  /// A response that carried DataDome's block signature (the `x-dd-b`
  /// header, or a 403 JSON body naming DataDome) but no usable detail.
  Unclassified {
    status_code: u16,
    raw_http_body: String,
  },
}

impl Error for DataDomeError {}

impl Display for DataDomeError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::CaptchaChallenge { challenge_url } => {
        write!(f, "DataDome CAPTCHA challenge; a browser must solve it to earn a new datadome cookie (challenge url: {})", challenge_url)
      }
      Self::Interstitial { challenge_url } => {
        write!(f, "DataDome device-check interstitial; a browser must pass it to earn a new datadome cookie (challenge url: {})", challenge_url)
      }
      Self::Blocked { maybe_block_url } => {
        write!(f, "DataDome blocked this client outright (no challenge offered){}",
          maybe_block_url.as_deref().map(|url| format!(" (block url: {url})")).unwrap_or_default())
      }
      Self::Unclassified { status_code, raw_http_body } => {
        write!(f, "DataDome rejected the request (status {}) with an unrecognized body: {}", status_code, raw_http_body)
      }
    }
  }
}

impl DataDomeError {
  /// A browser session can clear this (captcha / interstitial).
  pub fn is_challenge(&self) -> bool {
    matches!(self, Self::CaptchaChallenge { .. } | Self::Interstitial { .. })
  }

  /// Nothing short of a new identity will clear this.
  pub fn is_hard_block(&self) -> bool {
    matches!(self, Self::Blocked { .. })
  }

  /// The URL DataDome would send a browser to, when known.
  pub fn challenge_url(&self) -> Option<&str> {
    match self {
      Self::CaptchaChallenge { challenge_url } | Self::Interstitial { challenge_url } => Some(challenge_url),
      Self::Blocked { maybe_block_url } => maybe_block_url.as_deref(),
      Self::Unclassified { .. } => None,
    }
  }

  /// Challenges are a fingerprint/session problem we can act on (warn);
  /// hard blocks and mysteries need eyes (error).
  pub fn log_level(&self) -> Level {
    match self {
      Self::CaptchaChallenge { .. } | Self::Interstitial { .. } => Level::Warn,
      Self::Blocked { .. } | Self::Unclassified { .. } => Level::Error,
    }
  }

  /// Log at [`Self::log_level`] with a caller-supplied context (which host,
  /// which endpoint). One place to keep the wording consistent.
  pub fn log(&self, context: &str) {
    log::log!(self.log_level(), "[datadome] {}: {}", context, self);
  }
}
