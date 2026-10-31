use crate::endpoints::webhook::common::stripe_artcraft_webhook_error::StripeArtcraftWebhookError;
use crate::endpoints::webhook::common::webhook_event_log_summary::WebhookEventLogSummary;
use crate::utils::expand_ids::expand_customer_id::expand_customer_id;
use crate::utils::metadata::get_metadata_user_token::get_metadata_user_token;
use stripe_shared::Invoice;

// Handle event type: 'invoice.payment_failed'
pub fn invoice_payment_failed_extractor(invoice: &Invoice) -> Result<WebhookEventLogSummary, StripeArtcraftWebhookError> {
  let maybe_invoice_id = invoice.id.as_ref().map(|id| id.to_string());

  let paid_status = invoice.status;

  // NB: We'll need this to send them to the "customer portal", which is how they can modify
  // or cancel their subscriptions.
  let maybe_stripe_customer_id  = invoice.customer
      .as_ref()
      .map(|c| expand_customer_id(c));

  // NB: Our internal user token.
  let maybe_user_token = invoice.metadata
      .as_ref()
      .map(|metadata| get_metadata_user_token(metadata))
      .flatten();

  Ok(WebhookEventLogSummary {
    maybe_user_token,
    maybe_event_entity_id: maybe_invoice_id,
    maybe_stripe_customer_id,
    action_was_taken: false,
    should_ignore_retry: false,
  })
}
