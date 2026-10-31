use stripe::Charge;

use crate::stripe::helpers::common_metadata_keys::METADATA_USER_TOKEN;
use crate::stripe::helpers::expand_customer_id::expand_customer_id;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::stripe_webhook_error::StripeWebhookError;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::stripe_webhook_summary::StripeWebhookSummary;

// Handle event type: 'charge.succeeded'
pub fn charge_succeeded_handler(charge: &Charge) -> Result<StripeWebhookSummary, StripeWebhookError> {
  let charge_id = charge.id.to_string();

  let charge_status = charge.status;

  // NB: We'll need this to send them to the "customer portal", which is how they can modify
  // or cancel their subscriptions.
  let maybe_stripe_customer_id  = charge.customer
      .as_ref()
      .map(|c| expand_customer_id(c));

  // NB: Our internal user token.
  let maybe_user_token = charge.metadata.get(METADATA_USER_TOKEN)
      .map(|t| t.to_string());

  Ok(StripeWebhookSummary {
    maybe_user_token,
    maybe_event_entity_id: Some(charge_id),
    maybe_stripe_customer_id,
    action_was_taken: false,
    should_ignore_retry: false,
  })
}
