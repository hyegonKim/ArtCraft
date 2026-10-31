use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A concrete, ready-to-send `x-statsig-id`, minted for one endpoint at one
/// time from [`StatsigMaterial`](crate::StatsigMaterial).
///
/// The embedded timestamp ages out, so a `Statsig` is short-lived — regenerate
/// per request rather than caching it for long.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Statsig {
  /// The header value to send as `x-statsig-id`.
  pub statsig: String,

  /// When it was generated (this is the timestamp baked into the signature).
  pub generated_at: DateTime<Utc>,

  /// The HTTP method the signature is bound to (uppercase).
  pub method: String,

  /// The request path the signature is bound to.
  pub path: String,
}
