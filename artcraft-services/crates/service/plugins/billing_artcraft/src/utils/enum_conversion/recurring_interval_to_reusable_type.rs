use reusable_types::stripe::stripe_recurring_interval::StripeRecurringInterval;
use stripe_shared::RecurringInterval;

pub fn recurring_interval_to_reusable_type(recurring_interval: RecurringInterval) -> StripeRecurringInterval{
  match recurring_interval {
    RecurringInterval::Day => StripeRecurringInterval::Day,
    RecurringInterval::Month => StripeRecurringInterval::Month,
    RecurringInterval::Week => StripeRecurringInterval::Week,
    RecurringInterval::Year => StripeRecurringInterval::Year,
  }
}

#[cfg(test)]
pub mod tests {
  use reusable_types::stripe::stripe_recurring_interval::StripeRecurringInterval;
  use stripe_shared::RecurringInterval;
  use crate::utils::enum_conversion::recurring_interval_to_reusable_type::recurring_interval_to_reusable_type;

  #[test]
  fn test_type_conversion() {
    assert_eq!(recurring_interval_to_reusable_type(RecurringInterval::Day), StripeRecurringInterval::Day);
    assert_eq!(recurring_interval_to_reusable_type(RecurringInterval::Month), StripeRecurringInterval::Month);
    assert_eq!(recurring_interval_to_reusable_type(RecurringInterval::Week), StripeRecurringInterval::Week);
    assert_eq!(recurring_interval_to_reusable_type(RecurringInterval::Year), StripeRecurringInterval::Year);
  }
}
