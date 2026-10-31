use crate::endpoints::webhook::common::webhook_event_log_summary::WebhookEventLogSummary;
use log::info;
use stripe_webhook::{Event, EventObject};

// Returns a response if we know we want to ignore this event type.
// This serves primarily as documentation for why we're ignoring certain events.
pub fn ignore_known_unwanted_events(webhook_payload: &Event) -> Option<WebhookEventLogSummary> {

  match webhook_payload.data.object {
    EventObject::CheckoutSessionCompleted(_) => {
      // We don't use this to provision service - we use `invoice.paid` instead.
      // 
      // Checkout Session Completed `payment_status = {paid, unpaid, no_payment_required}` 
      // will let us know if the funds are in our account under SOME circumstances. 
      //
      // NOTE however that:
      //   (1) this will not be `paid` for free trials, whereas `invoice.paid` events will fire for free trials.
      //   (2) async methods like ACH will have the status `unpaid` until the payment clears.
      //
      // Thus, we really don't want to use this event to provision service.
    }
    EventObject::ChargeSucceeded(_) => {
      // charge.succeeded - building block event and also used for older legacy integrations.
      // Use `invoice.paid` as billing success signal for subscriptions.
      // Use `payment_intent.succeeded` for one-off and add-on purchases.
    }
    EventObject::ChargeUpdated(_) => {
      // This doesn't track the payment status, and we don't care about fraud, risk,
      // receipt, etc. updates.
    }
    EventObject::CustomerCreated(_) |
    EventObject::CustomerDeleted(_) |
    EventObject::CustomerUpdated(_) => {
      // We don't need to know about Stripe customer object metadata changes,
      // eg. stripe email change. Unfortunately, we also can't directly associate
      // metadata with customers in a simple API call to create a checkout or portal
      // session, so these objects are kind of useless.
    }
    EventObject::PaymentIntentCreated(_) => {
      // Not very actionable for us. We only care about successful payments.
    }
    EventObject::PaymentMethodAttached(_) => {
      // We don't need to know about card additions or changes.
    }
    EventObject::InvoiceCreated(_) => {
      // Not relevant for our business model. Draft invoices can change.
    }
    EventObject::InvoicePaymentSucceeded(_) => {
      // Not as useful as the `invoice.paid` event, because this can describe
      // retry, partial payments, partial captures, etc. It doesn't mean the
      // payment went through or that the payment lifecycle is complete.
      // Use `invoice.paid` instead.
    }
    EventObject::InvoiceUpdated(_) => {
      // Not relevant for our business model. We're not changing line items
      // during the billing cycle.
    }
    EventObject::InvoiceFinalized(_) => {
      // Not relevant for our business model. This happens before payment.
    }

    // EventObject::CheckoutSessionAsyncPaymentFailed(_) => {}
    // EventObject::CheckoutSessionAsyncPaymentSucceeded(_) => {}
    // EventObject::CheckoutSessionExpired(_) => {}

    // EventObject::CustomerSubscriptionPendingUpdateApplied(_) => {}
    // EventObject::CustomerSubscriptionPendingUpdateExpired(_) => {}
    // EventObject::CustomerSubscriptionTrialWillEnd(_) => {}

    // EventObject::InvoiceDeleted(_) => {}
    // EventObject::InvoiceFinalizationFailed(_) => {}
    // EventObject::InvoiceFinalized(_) => {}
    // EventObject::InvoiceItemCreated(_) => {}
    // EventObject::InvoiceItemDeleted(_) => {}
    // EventObject::InvoiceItemUpdated(_) => {}
    // EventObject::InvoiceMarkedUncollectible(_) => {}
    // EventObject::InvoicePaymentActionRequired(_) => {}
    // EventObject::InvoiceSent(_) => {}
    // EventObject::InvoiceUpcoming(_) => {}
    // EventObject::InvoiceUpdated(_) => {}
    // EventObject::InvoiceVoided(_) => {}

    // EventType::PaymentIntentAmountCapturableUpdated => {}
    // EventType::PaymentIntentCanceled => {}
    // EventType::PaymentIntentCreated => {}
    // EventType::PaymentIntentPartiallyFunded => {}
    // EventType::PaymentIntentPaymentFailed => {}
    // EventType::PaymentIntentProcessing => {}
    // EventType::PaymentIntentRequiresAction => {}
    // EventType::PaymentIntentRequiresCapture => {}

    // Ignoring these types (NOTE: This is from the old async-stripe version):
    //   Account* (6),
    //   ApplicationFee* (2),
    //   BalanceAvailable (1),
    //   BillingPortal* (2),
    //   CapabilityCreated (1),
    //   Charge* (13),
    //   [CheckoutSession* handled above],
    //   Coupon* (3),
    //   CreditNote* (3),
    //   CustomerDiscount* (3),
    //   CustomerSource* (4),
    //   [CustomerSubscription* handled above],
    //   CreditTax* (3),
    //   FileCreated (1),
    //   IdentityVerification* (6),
    //   [Invoice* handled above],
    //   IssuingAuthorization* (3),
    //   IssuingCard* (2),
    //   IssuingCardholder* (2),
    //   IssuingDispute* (2),
    //   IssuingTransaction* (2),
    //   MandateUpdated (1),
    //   Order* (6),
    //   [PaymentIntent* handled above],
    //   PaymentLink* (2),
    //   PaymentMethod* (4),
    //   Payout* (5),
    //   Person* (3),
    //   Plan* (3),
    //   Price* (3),
    //   Product* (3),
    //   PromotionCode* (2),
    //   Quote* (4),
    //   RadarEarlyFraudWarning* (2),
    //   Recipient* (3),
    //   ReportingReport* (3),
    //   Review* (2),
    //   SetupIntent* (5), -- N/A; collecting payments in the future we don't yet have information for (eg. crowdfunding, rental car, utility bill)
    //   SubscriptionSchedule* (7), -- N/A; backdate subscriptions or schedule one to start later (eg. for physical goods, like a newspaper)
    //   SigmaScheduledQueryRunCreated (1),
    //   Sku (3),
    //   Source* (7),
    //   TaxRate* (2),
    //   TerminalReader* (2),
    //   TestHelpersTestClock* (5),
    //   Topup* (5),
    //   Transfer* (5),

    _ => return None,
  }

  info!("Ignoring irrelevant event type: {}", webhook_payload.type_);

  Some(WebhookEventLogSummary {
    maybe_user_token: None,
    maybe_event_entity_id: None,
    maybe_stripe_customer_id: None,
    action_was_taken: false,
    should_ignore_retry: true,
  })
}
