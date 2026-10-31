use stripe::Invoice;

use crate::stripe::helpers::common_metadata_keys::METADATA_USER_TOKEN;
use crate::stripe::helpers::expand_customer_id::expand_customer_id;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::stripe_webhook_error::StripeWebhookError;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::stripe_webhook_summary::StripeWebhookSummary;

// Handle event type: 'invoice.payment_succeeded'
pub fn invoice_payment_succeeded_handler(invoice: &Invoice) -> Result<StripeWebhookSummary, StripeWebhookError> {
  let invoice_id = invoice.id.to_string();

  let paid_status = invoice.status;

  // NB: We'll need this to send them to the "customer portal", which is how they can modify
  // or cancel their subscriptions.
  let maybe_stripe_customer_id  = invoice.customer
      .as_ref()
      .map(|c| expand_customer_id(c));

  // NB: Our internal user token.
  let maybe_user_token = invoice.metadata
      .as_ref()
      .and_then(|m| m.get(METADATA_USER_TOKEN).map(|t| t.to_string()));

  Ok(StripeWebhookSummary {
    maybe_user_token,
    maybe_event_entity_id: Some(invoice_id),
    maybe_stripe_customer_id,
    action_was_taken: false,
    should_ignore_retry: false,
  })
}
