use stripe::Customer;

use crate::stripe::helpers::common_metadata_keys::METADATA_USER_TOKEN;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::stripe_webhook_error::StripeWebhookError;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::stripe_webhook_summary::StripeWebhookSummary;

// Handle event type: 'customer.deleted'
pub fn customer_deleted_handler(customer: &Customer) -> Result<StripeWebhookSummary, StripeWebhookError> {
  // NB: We'll need this to send them to the "customer portal", which is how they can modify
  // or cancel their subscriptions.
  let customer_id = customer.id.to_string();

  // NB: Our internal user token.
  let maybe_user_token = customer.metadata
      .as_ref()
      .and_then(|m| m.get(METADATA_USER_TOKEN).map(|t| t.to_string()));

  Ok(StripeWebhookSummary {
    maybe_user_token,
    maybe_event_entity_id: Some(customer_id.clone()),
    maybe_stripe_customer_id: Some(customer_id.clone()),
    action_was_taken: false,
    should_ignore_retry: false,
  })
}
