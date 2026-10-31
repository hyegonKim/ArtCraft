use crate::datatypes::api::baggage::Baggage;
use crate::datatypes::api::sentry_trace::SentryTrace;
use crate::datatypes::api::user_email::UserEmail;
use crate::datatypes::api::user_id::UserId;

/// Per-session values Grok's web app reads out of `index.html`. The
/// `x-statsig-id` signature is *not* here — it is minted by a real browser (see
/// the `grok_consumer_statsig` crate).
#[derive(Clone)]
pub struct GrokClientSecrets {
  /// `<meta>` tag tracing data from index.html.
  pub baggage: Baggage,

  /// `<meta>` tag tracing data from index.html.
  pub sentry_trace: SentryTrace,

  /// From index.html. Typically needed to generate URLs.
  pub user_id: UserId,

  /// From index.html. Not needed, but returned alongside other details.
  pub user_email: Option<UserEmail>,
}
