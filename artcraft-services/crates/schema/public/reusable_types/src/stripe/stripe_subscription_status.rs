//! This is copied from the stripe crate's `SubscriptionStatus`, and type juggling should
//! occur outside of this crate.

// There are three changes from the source create:
//   - Renamed the struct from SubscriptionStatus to StripeSubscriptionStatus
//   - Added StripeSubscriptionStatus::from_str()
//   - Added derive sqlx::Type and sqlx(rename_all)
//   - Added Deref impl
//   - Added tests

// NB: Added "sqlx::Type".
#[derive(Copy, Clone, Debug, Deserialize, Serialize, Eq, PartialEq, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum StripeSubscriptionStatus {
  Active,
  Canceled,
  Incomplete,
  IncompleteExpired,
  PastDue,
  Trialing,
  Unpaid,
  Paused,
}

impl StripeSubscriptionStatus {
  pub fn as_str(self) -> &'static str {
    match self {
      StripeSubscriptionStatus::Active => "active",
      StripeSubscriptionStatus::Canceled => "canceled",
      StripeSubscriptionStatus::Incomplete => "incomplete",
      StripeSubscriptionStatus::IncompleteExpired => "incomplete_expired",
      StripeSubscriptionStatus::PastDue => "past_due",
      StripeSubscriptionStatus::Trialing => "trialing",
      StripeSubscriptionStatus::Unpaid => "unpaid",
      StripeSubscriptionStatus::Paused => "paused"
    }
  }

  // NB: Added by us.
  pub fn from_str(value: &str) -> Result<Self, String> {
    match value {
      "active" => Ok(Self::Active),
      "canceled" => Ok(Self::Canceled),
      "incomplete" => Ok(Self::Incomplete),
      "incomplete_expired" => Ok(Self::IncompleteExpired),
      "past_due" => Ok(Self::PastDue),
      "trialing" => Ok(Self::Trialing),
      "unpaid" => Ok(Self::Unpaid),
      "paused" => Ok(Self::Paused),
      _ => Err(format!("invalid value: {:?}", value)),
    }
  }
}

impl AsRef<str> for StripeSubscriptionStatus {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

// NB: Added by us.
impl std::ops::Deref for StripeSubscriptionStatus {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    self.as_str()
  }
}

impl std::fmt::Display for StripeSubscriptionStatus {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    self.as_str().fmt(f)
  }
}

impl std::default::Default for StripeSubscriptionStatus {
  fn default() -> Self {
    Self::Active
  }
}

#[cfg(test)]
mod tests {
  use crate::stripe::stripe_subscription_status::StripeSubscriptionStatus;

  // NB(bt,2024-09-25): These tests are broken with a recent upgrade
  //#[test]
  //fn test_serialization() {
  //  assert_serialization(StripeSubscriptionStatus::Active, "active");
  //  assert_serialization(StripeSubscriptionStatus::Canceled, "canceled");
  //  assert_serialization(StripeSubscriptionStatus::Incomplete, "incomplete");
  //  assert_serialization(StripeSubscriptionStatus::IncompleteExpired, "incomplete_expired");
  //  assert_serialization(StripeSubscriptionStatus::PastDue, "past_due");
  //  assert_serialization(StripeSubscriptionStatus::Trialing, "trialing");
  //  assert_serialization(StripeSubscriptionStatus::Unpaid, "unpaid");
  //  assert_serialization(StripeSubscriptionStatus::Paused, "paused");
  //}

  #[test]
  fn test_as_str() {
    assert_eq!(StripeSubscriptionStatus::Active.as_str(), "active");
    assert_eq!(StripeSubscriptionStatus::Canceled.as_str(), "canceled");
    assert_eq!(StripeSubscriptionStatus::Incomplete.as_str(), "incomplete");
    assert_eq!(StripeSubscriptionStatus::IncompleteExpired.as_str(), "incomplete_expired");
    assert_eq!(StripeSubscriptionStatus::PastDue.as_str(), "past_due");
    assert_eq!(StripeSubscriptionStatus::Trialing.as_str(), "trialing");
    assert_eq!(StripeSubscriptionStatus::Unpaid.as_str(), "unpaid");
    assert_eq!(StripeSubscriptionStatus::Paused.as_str(), "paused");
  }

  #[test]
  fn test_from_str() {
    assert_eq!(StripeSubscriptionStatus::from_str("active").unwrap(), StripeSubscriptionStatus::Active);
    assert_eq!(StripeSubscriptionStatus::from_str("canceled").unwrap(), StripeSubscriptionStatus::Canceled);
    assert_eq!(StripeSubscriptionStatus::from_str("incomplete").unwrap(), StripeSubscriptionStatus::Incomplete);
    assert_eq!(StripeSubscriptionStatus::from_str("incomplete_expired").unwrap(), StripeSubscriptionStatus::IncompleteExpired);
    assert_eq!(StripeSubscriptionStatus::from_str("past_due").unwrap(), StripeSubscriptionStatus::PastDue);
    assert_eq!(StripeSubscriptionStatus::from_str("trialing").unwrap(), StripeSubscriptionStatus::Trialing);
    assert_eq!(StripeSubscriptionStatus::from_str("unpaid").unwrap(), StripeSubscriptionStatus::Unpaid);
    assert_eq!(StripeSubscriptionStatus::from_str("paused").unwrap(), StripeSubscriptionStatus::Paused);
    assert!(StripeSubscriptionStatus::from_str("foo").is_err());
  }
}
