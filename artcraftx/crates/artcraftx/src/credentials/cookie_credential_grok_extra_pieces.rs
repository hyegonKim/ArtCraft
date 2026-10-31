use chrono::{DateTime, Utc};
use grok_consumer_statsig::StatsigMaterial;
use serde_derive::{Deserialize, Serialize};

/// How long a captured statsig (and its material) is considered current before
/// the app should re-capture. Single source of truth for the login flow, the
/// preemptive refresher, and the generation workers.
pub const STATSIG_REFRESH_MINUTES: i64 = 30;

/// Grok-specific extras stored alongside the cookies in a
/// [`CookieCredential`](crate::credentials::cookie_credential::CookieCredential).
///
/// Holds two things captured from the user's own grok.com session:
///
/// 1. The optional statsig *prerequisite* material (the seed a fresh
///    `x-statsig-id` could be minted from — see [`StatsigMaterial`]).
/// 2. The *latest* browser-minted `x-statsig-id` itself, with the request it
///    signed and when it was captured / expires. This is what generation
///    requests actually send (see [`Self::latest_statsig_if_fresh`]).
///
/// NB: TOML field order matters — scalars first, then the nested tables.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CookieCredentialGrokExtraPieces {
  /// When the statsig material was captured.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub statsig_fetched_at: Option<DateTime<Utc>>,

  /// When the statsig material should be re-captured
  /// ([`STATSIG_REFRESH_MINUTES`] after fetch).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub statsig_refresh_at: Option<DateTime<Utc>>,

  /// The reusable statsig material (currently just the captured seed).
  /// NB: a table, so it must stay after the scalar timestamp fields.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub statsig_material: Option<StatsigMaterial>,

  /// The latest browser-minted `x-statsig-id`, plus timing.
  /// NB: a table, so it must stay after the scalar fields.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub latest_statsig: Option<LatestGrokStatsig>,
}

/// A browser-minted `x-statsig-id` as captured from the page, with the request
/// it signed and its validity window.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LatestGrokStatsig {
  /// The raw `x-statsig-id` header value.
  pub statsig_id: String,

  /// HTTP method of the request the page signed (e.g. `POST`).
  pub method: String,

  /// Path of the request the page signed (e.g. `/rest/app-chat/conversations/new`).
  pub path: String,

  /// When the page minted it.
  pub captured_at: DateTime<Utc>,

  /// When it should no longer be sent ([`STATSIG_REFRESH_MINUTES`] after capture).
  pub expires_at: DateTime<Utc>,
}

/// Everything one passive capture yields: the seed material and the signed
/// header it came from. Produced by the login / refresh webviews.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokStatsigCapture {
  pub material: StatsigMaterial,
  pub statsig_id: String,
  pub method: String,
  pub path: String,
  pub captured_at: DateTime<Utc>,
}

impl CookieCredentialGrokExtraPieces {
  /// Stamp a fresh capture with fetch/expiry times [`STATSIG_REFRESH_MINUTES`]
  /// into the future.
  pub fn fresh(capture: GrokStatsigCapture, now: DateTime<Utc>) -> Self {
    let expires_at = now + chrono::Duration::minutes(STATSIG_REFRESH_MINUTES);
    Self {
      statsig_fetched_at: Some(now),
      statsig_refresh_at: Some(expires_at),
      statsig_material: Some(capture.material),
      latest_statsig: Some(LatestGrokStatsig {
        statsig_id: capture.statsig_id,
        method: capture.method,
        path: capture.path,
        captured_at: capture.captured_at,
        expires_at,
      }),
    }
  }

  /// The latest captured `x-statsig-id`, if there is one and it hasn't expired.
  pub fn latest_statsig_if_fresh(&self, now: DateTime<Utc>) -> Option<&str> {
    self.latest_statsig
        .as_ref()
        .filter(|latest| now < latest.expires_at)
        .map(|latest| latest.statsig_id.as_str())
  }

  /// Whether the stored statsig is missing or past its refresh time, so it
  /// should be re-captured. Both the material and the latest signed header must
  /// be present and current.
  pub fn is_stale(&self, now: DateTime<Utc>) -> bool {
    let material_stale = match (&self.statsig_material, self.statsig_refresh_at) {
      (Some(_), Some(refresh_at)) => now >= refresh_at,
      _ => true, // no material, or no refresh time recorded
    };
    material_stale || self.latest_statsig_if_fresh(now).is_none()
  }
}

/// Whether a cookie credential's Grok statsig data is absent or stale and
/// should be re-captured. `None` (no `grok_data` at all) counts as stale.
pub fn grok_statsig_needs_refresh(
  grok_data: Option<&CookieCredentialGrokExtraPieces>,
  now: DateTime<Utc>,
) -> bool {
  grok_data.map_or(true, |data| data.is_stale(now))
}

#[cfg(test)]
mod tests {
  use super::*;

  const STATSIG_ID: &str = "XbHvyYh7hmNc+8sVAGnrpcuHN/MK+mgN63cmnBljOJaoCKu6zTPVw6C7HVv1wculKCNmYVum+L1h0IjFmtyZD/C53M+ZXg";

  fn capture(now: DateTime<Utc>) -> GrokStatsigCapture {
    GrokStatsigCapture {
      material: StatsigMaterial::from_seed_b64("7LKU1SbbPgGmlkhdNLb4ltpqrlenNVC2KnvBRD5ly/VV9ueQboie/eZABqiclvh1"),
      statsig_id: STATSIG_ID.to_string(),
      method: "POST".to_string(),
      path: "/rest/app-chat/conversations/new".to_string(),
      captured_at: now,
    }
  }

  #[test]
  fn missing_grok_data_is_stale() {
    assert!(grok_statsig_needs_refresh(None, Utc::now()));
  }

  #[test]
  fn fresh_capture_is_not_stale_until_refresh_time() {
    let now = Utc::now();
    let data = CookieCredentialGrokExtraPieces::fresh(capture(now), now);
    assert!(!data.is_stale(now));
    assert!(!data.is_stale(now + chrono::Duration::minutes(STATSIG_REFRESH_MINUTES - 1)));
    assert!(data.is_stale(now + chrono::Duration::minutes(STATSIG_REFRESH_MINUTES)));
  }

  #[test]
  fn latest_statsig_is_served_only_while_fresh() {
    let now = Utc::now();
    let data = CookieCredentialGrokExtraPieces::fresh(capture(now), now);
    assert_eq!(data.latest_statsig_if_fresh(now), Some(STATSIG_ID));
    assert_eq!(
      data.latest_statsig_if_fresh(now + chrono::Duration::minutes(STATSIG_REFRESH_MINUTES)),
      None,
    );
  }

  #[test]
  fn material_without_latest_statsig_is_stale() {
    let now = Utc::now();
    let data = CookieCredentialGrokExtraPieces {
      statsig_fetched_at: Some(now),
      statsig_refresh_at: Some(now + chrono::Duration::minutes(STATSIG_REFRESH_MINUTES)),
      statsig_material: Some(capture(now).material),
      latest_statsig: None,
    };
    assert!(data.is_stale(now));
    assert_eq!(data.latest_statsig_if_fresh(now), None);
  }

  #[test]
  fn material_without_refresh_time_is_stale() {
    let now = Utc::now();
    let data = CookieCredentialGrokExtraPieces {
      statsig_fetched_at: None,
      statsig_refresh_at: None,
      statsig_material: Some(capture(now).material),
      latest_statsig: None,
    };
    assert!(data.is_stale(now));
  }
}
