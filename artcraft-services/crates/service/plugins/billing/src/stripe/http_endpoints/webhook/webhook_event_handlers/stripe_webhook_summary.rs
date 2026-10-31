
/// Reports common information from the webhook handlers
pub struct StripeWebhookSummary {
  /// If we recorded our own internal user ID as Stripe metadata, this passes it upstream.
  pub maybe_user_token: Option<String>,

  /// This is the core entity type associated with the webhook event.
  /// We pass this upstream so we can record it and look it up later for debugging.
  pub maybe_event_entity_id: Option<String>,

  /// This is the stripe customer ID, if it was associated with the event.
  pub maybe_stripe_customer_id: Option<String>,

  /// Whether we took any sort of action in response to the webhook.
  /// Not all event types have handlers yet, and even so, sometimes we may choose to do nothing.
  pub action_was_taken: bool,

  /// Whether any retries should be dropped.
  /// This helps us handle event with idempotency in the event Stripe replays them (and it can!)
  pub should_ignore_retry: bool,
}
