use std::ops::{Add, Sub};

use chrono::{Duration, NaiveDateTime};
use once_cell::sync::Lazy;

use reusable_types::stripe::stripe_subscription_status::StripeSubscriptionStatus;

use crate::stripe::http_endpoints::webhook::webhook_event_handlers::customer_subscription::subscription_event_extractor::SubscriptionSummary;

// Push back the expiration date.
static SUBSCRIPTION_GRACE_DAYS : Lazy<Duration> = Lazy::new(|| Duration::days(2));

// Set canceled subscriptions back in time over a year, which is the maximum billing period + grace.
static BACKDATE_TERMINATION_DAYS : Lazy<Duration> = Lazy::new(|| Duration::days(370));

#[inline]
pub fn calculate_subscription_end_date(subscription_summary: &SubscriptionSummary) -> NaiveDateTime {
  if subscription_summary.stripe_subscription_status == StripeSubscriptionStatus::Canceled {
    // If it's cancelled, the user is done and the service should be removed.
    // NB: Subscriptions set to cancel in the future (from the admin UI) are still "active".
    return subscription_summary.maybe_canceled_at
        .unwrap_or(subscription_summary.current_billing_period_end)
        .sub(*BACKDATE_TERMINATION_DAYS); // Just to be sure.
  }

  let mut subscription_end = subscription_summary.current_billing_period_end;

  if !subscription_summary.cancel_at_period_end {
    subscription_end = subscription_end.add(*SUBSCRIPTION_GRACE_DAYS);
  }

  subscription_end
}
