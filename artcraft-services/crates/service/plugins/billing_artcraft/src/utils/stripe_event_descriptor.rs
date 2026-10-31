use std::fmt::{Debug, Display, Formatter};

/// Convenience type for logging, error messages, etc.
/// NB: `Debug` not derived.
#[derive(Clone)]
pub struct StripeEventDescriptor {
  /// Stripe primary key for event, eg. `evt_1S592qEobp4xy4TljPApEJXe`
  pub stripe_event_id: String,

  /// Type of event, eg. `payment_intent.succeeded`
  pub stripe_event_type: String,
}

impl Display for StripeEventDescriptor {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{} ({})", self.stripe_event_type, self.stripe_event_id)
  }
}

impl Debug for StripeEventDescriptor {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{} ({})", self.stripe_event_type, self.stripe_event_id)
  }
}
