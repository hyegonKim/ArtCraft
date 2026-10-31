use crate::billing_action_fulfillment::artcraft_billing_action::ArtcraftBillingAction;
use crate::endpoints::webhook::common::webhook_event_log_summary::WebhookEventLogSummary;

pub struct EnrichedWebhookEvent {
  /// Higher-level events that we care about handling.
  /// Not all webhook requests have them.
  pub maybe_billing_action: Option<ArtcraftBillingAction>,
  
  /// Details of what we'll write to the webhook event logs.
  /// In addition to important events, this includes some no-op events,
  /// various irrelevant events, errors, etc.
  pub webhook_event_log_summary: WebhookEventLogSummary,
}

impl EnrichedWebhookEvent {
  
  pub fn from_actionless_log(summary: WebhookEventLogSummary) -> Self {
    EnrichedWebhookEvent {
      maybe_billing_action: None,
      webhook_event_log_summary: summary,
    }
  }
}
