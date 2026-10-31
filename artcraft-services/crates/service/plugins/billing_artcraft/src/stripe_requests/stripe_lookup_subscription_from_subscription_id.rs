use crate::configs::stripe_client_retry_strategy::STRIPE_CLIENT_RETRY_STRATEGY;
use crate::stripe_requests::stripe_lookup_purchase_from_payment_intent_success::PurchaseSummary;
use crate::utils::enum_conversion::recurring_interval_to_reusable_type::recurring_interval_to_reusable_type;
use crate::utils::enum_conversion::subscription_status_to_reusable_type::subscription_status_to_reusable_type;
use crate::utils::expand_ids::expand_customer_id::expand_customer_id;
use crate::utils::expand_ids::expand_product_id::expand_product_id;
use crate::utils::metadata::get_metadata_user_token::get_metadata_user_token;
use anyhow::anyhow;
use chrono::NaiveDateTime;
use log::{error, warn};
use reusable_types::stripe::stripe_recurring_interval::StripeRecurringInterval;
use reusable_types::stripe::stripe_subscription_status::StripeSubscriptionStatus;
use stripe::{Client, StripeRequest};
use stripe_billing::subscription::RetrieveSubscription;
use stripe_checkout::checkout_session::{ListCheckoutSession, ListLineItemsCheckoutSession};
use stripe_shared::{CheckoutSession, CheckoutSessionItem, PriceType, Subscription, SubscriptionStatus};
use stripe_types::List;
use tokens::tokens::users::UserToken;

// TODO: Code reuse with `subscription_event_extractor`.
pub struct SubscriptionAndProduct {
  /// Our own internal user token, if it was attached to the subscription.
  pub maybe_user_token: Option<UserToken>,

  // Stripe foreign keys

  pub stripe_subscription_id: String,
  pub stripe_product_id: String,
  pub stripe_customer_id: String,
  pub stripe_price_id: String,

  /// The line item within the subscription (our subscriptions only have one line item)
  pub stripe_subscription_item_id: String,

  /// Stripe production flag.
  pub stripe_is_production: bool,

  /// The state of the subscription: active, cancelled, and other states.
  pub stripe_subscription_status: StripeSubscriptionStatus,

  /// We really only care about this when subscriptions are scheduled to be terminated.
  /// Do we let the user continue receiving service in a grace period, or cut it off immediately
  /// at the scheduled service end date?
  /// (This avoids adding extra service days as a grace period.)
  pub cancel_at_period_end: bool,

  pub subscription_is_active: bool,
  pub subscription_interval: StripeRecurringInterval,

  // Which day of the month / month of the year to anchor the subscription against.
  // See the Stripe docs.
  pub stripe_billing_cycle_anchor: NaiveDateTime,

  /// When the subscription was "created" in Stripe (including any backdating)
  pub subscription_start_date: NaiveDateTime,

  // The updated billing period for the subscription
  pub current_billing_period_start: NaiveDateTime,
  pub current_billing_period_end: NaiveDateTime,

  // When the subscription will be (or was) canceled
  pub maybe_cancel_at: Option<NaiveDateTime>,
  pub maybe_canceled_at: Option<NaiveDateTime>,
}

pub async fn stripe_lookup_subscription_from_subscription_id(
  subscription_id: &str,
  stripe_client: &Client
) -> anyhow::Result<SubscriptionAndProduct> {

  let subscription = RetrieveSubscription::new(subscription_id)
      .build()
      .customize::<Subscription>()
      .request_strategy(STRIPE_CLIENT_RETRY_STRATEGY)
      .send(stripe_client)
      .await
      .map_err(|err| {
        error!("Stripe Error looking up subscription for subscription id{}: {:?}", &subscription_id, err);
        err
      })?;

  if subscription.items.data.len() != 1 {
    warn!("Unexpected number of line items: {} for subscription {}", subscription.items.data.len(), subscription_id);
  }

  let line_item = subscription
      .items
      .data
      .get(0)
      .ok_or_else(|| anyhow::anyhow!("No line items found in subscription."))?;

  let subscription_item_id = line_item.id.to_string();
  
  let price = &line_item.price;

  match price.type_ {
    PriceType::OneTime => {}
    PriceType::Recurring => {}
  }

  let price_id = price.id.to_string();
  let product_id = expand_product_id(&price.product);
  let customer_id = expand_customer_id(&subscription.customer);

  let maybe_user_token = get_metadata_user_token(&subscription.metadata);

  let recurring = match &price.recurring {
    None => return Err(anyhow!("Could not get interval in subscription {}", subscription_id)),
    Some(recurring) => recurring,
  };

  // NB: period start and end dates were moved out of the root object and into the line items
  let start_date = NaiveDateTime::from_timestamp(subscription.start_date, 0);
  let period_start = NaiveDateTime::from_timestamp(line_item.current_period_start, 0);
  let period_end = NaiveDateTime::from_timestamp(line_item.current_period_end, 0);

  let stripe_billing_cycle_anchor = NaiveDateTime::from_timestamp(subscription.billing_cycle_anchor, 0);

  let maybe_cancel_at = subscription.cancel_at.map(|t| NaiveDateTime::from_timestamp(t, 0));
  let maybe_canceled_at = subscription.canceled_at.map(|t| NaiveDateTime::from_timestamp(t, 0));

  //let quantity = line_item.quantity.unwrap_or(1);

  Ok(SubscriptionAndProduct {
    stripe_subscription_id: subscription_id.to_string(),
    stripe_subscription_item_id: subscription_item_id,
    stripe_product_id: product_id,
    stripe_customer_id: customer_id,
    stripe_price_id: price_id,
    stripe_is_production: subscription.livemode,
    stripe_subscription_status: subscription_status_to_reusable_type(subscription.status),
    maybe_user_token,
    subscription_is_active: subscription.status == SubscriptionStatus::Active,
    subscription_interval: recurring_interval_to_reusable_type(recurring.interval),
    subscription_start_date: start_date,
    current_billing_period_start: period_start,
    current_billing_period_end: period_end,
    stripe_billing_cycle_anchor,
    maybe_cancel_at,
    maybe_canceled_at,
    cancel_at_period_end: false,
  })
}
