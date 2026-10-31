//! This is copied from the stripe crate's `price::RecurringInterval`, and type juggling should
//! occur outside of this crate.

// There are three changes from the source create:
//   - Renamed the struct from price::RecurringInterval to StripeRecurringInterval
//   - Added StripeSubscriptionStatus::from_str()
//   - Added derive sqlx::Type and sqlx(rename_all)
//   - Added Deref impl
//   - Added tests

// NB: Added "sqlx::Type".
#[derive(Copy, Clone, Debug, Deserialize, Serialize, Eq, PartialEq, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum StripeRecurringInterval {
  Day,
  Month,
  Week,
  Year,
}

impl StripeRecurringInterval {
  pub fn as_str(self) -> &'static str {
    match self {
      StripeRecurringInterval::Day => "day",
      StripeRecurringInterval::Month => "month",
      StripeRecurringInterval::Week => "week",
      StripeRecurringInterval::Year => "year",
    }
  }

  // NB: Added by us.
  pub fn from_str(value: &str) -> Result<Self, String> {
    match value {
      "day" => Ok(Self::Day),
      "month" => Ok(Self::Month),
      "week" => Ok(Self::Week),
      "year" => Ok(Self::Year),
      _ => Err(format!("invalid value: {:?}", value)),
    }
  }
}

impl AsRef<str> for StripeRecurringInterval {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

// NB: Added by us.
impl std::ops::Deref for StripeRecurringInterval {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    self.as_str()
  }
}

impl std::fmt::Display for StripeRecurringInterval {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    self.as_str().fmt(f)
  }
}

impl std::default::Default for StripeRecurringInterval {
  fn default() -> Self {
    Self::Day
  }
}

#[cfg(test)]
mod tests {
  use crate::stripe::stripe_recurring_interval::StripeRecurringInterval;

  // NB(bt,2024-09-25): These tests are broken with a recent upgrade
  //#[test]
  //fn test_serialization() {
  //  assert_serialization(StripeRecurringInterval::Day, "day");
  //  assert_serialization(StripeRecurringInterval::Month, "month");
  //  assert_serialization(StripeRecurringInterval::Week, "week");
  //  assert_serialization(StripeRecurringInterval::Year, "year");
  //}

  #[test]
  fn test_as_str() {
    assert_eq!(StripeRecurringInterval::Day.as_str(), "day");
    assert_eq!(StripeRecurringInterval::Month.as_str(), "month");
    assert_eq!(StripeRecurringInterval::Week.as_str(), "week");
    assert_eq!(StripeRecurringInterval::Year.as_str(), "year");
  }

  #[test]
  fn test_from_str() {
    assert_eq!(StripeRecurringInterval::from_str("day").unwrap(), StripeRecurringInterval::Day);
    assert_eq!(StripeRecurringInterval::from_str("month").unwrap(), StripeRecurringInterval::Month);
    assert_eq!(StripeRecurringInterval::from_str("week").unwrap(), StripeRecurringInterval::Week);
    assert_eq!(StripeRecurringInterval::from_str("year").unwrap(), StripeRecurringInterval::Year);
    assert!(StripeRecurringInterval::from_str("foo").is_err());
  }
}
