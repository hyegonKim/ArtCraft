use reusable_types::stripe::stripe_subscription_status::StripeSubscriptionStatus;
use stripe_shared::SubscriptionStatus;

pub fn subscription_status_to_reusable_type(subscription_status: SubscriptionStatus) -> StripeSubscriptionStatus {
  match subscription_status {
    SubscriptionStatus::Active => StripeSubscriptionStatus::Active,
    SubscriptionStatus::Canceled => StripeSubscriptionStatus::Canceled,
    SubscriptionStatus::Incomplete => StripeSubscriptionStatus::Incomplete,
    SubscriptionStatus::IncompleteExpired => StripeSubscriptionStatus::IncompleteExpired,
    SubscriptionStatus::PastDue => StripeSubscriptionStatus::PastDue,
    SubscriptionStatus::Trialing => StripeSubscriptionStatus::Trialing,
    SubscriptionStatus::Unpaid => StripeSubscriptionStatus::Unpaid,
    SubscriptionStatus::Paused => StripeSubscriptionStatus::Paused,
  }
}

#[cfg(test)]
pub mod tests {
  use reusable_types::stripe::stripe_subscription_status::StripeSubscriptionStatus;
  use stripe_shared::SubscriptionStatus;
  use crate::utils::enum_conversion::subscription_status_to_reusable_type::subscription_status_to_reusable_type;

  #[test]
  fn test_type_conversion() {
    assert_eq!(subscription_status_to_reusable_type(SubscriptionStatus::Active), StripeSubscriptionStatus::Active);
    assert_eq!(subscription_status_to_reusable_type(SubscriptionStatus::Canceled), StripeSubscriptionStatus::Canceled);
    assert_eq!(subscription_status_to_reusable_type(SubscriptionStatus::Incomplete), StripeSubscriptionStatus::Incomplete);
    assert_eq!(subscription_status_to_reusable_type(SubscriptionStatus::IncompleteExpired), StripeSubscriptionStatus::IncompleteExpired);
    assert_eq!(subscription_status_to_reusable_type(SubscriptionStatus::PastDue), StripeSubscriptionStatus::PastDue);
    assert_eq!(subscription_status_to_reusable_type(SubscriptionStatus::Trialing), StripeSubscriptionStatus::Trialing);
    assert_eq!(subscription_status_to_reusable_type(SubscriptionStatus::Unpaid), StripeSubscriptionStatus::Unpaid);
    assert_eq!(subscription_status_to_reusable_type(SubscriptionStatus::Paused), StripeSubscriptionStatus::Paused);
  }
}
