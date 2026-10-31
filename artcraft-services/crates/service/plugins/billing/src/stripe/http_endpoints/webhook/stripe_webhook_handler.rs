use actix_web::{HttpRequest, HttpResponse, web};
use actix_web::web::Bytes;
use chrono::NaiveDateTime;
use log::{error, info, warn};
use sqlx::MySqlPool;
use stripe::{EventObject, EventType, Webhook};

use http_server_common::request::get_request_header_optional::get_request_header_optional;
use mysql_queries::queries::billing::stripe::get_stripe_webhook_event_log_by_id::get_stripe_webhook_event_log_by_id;
use mysql_queries::queries::billing::stripe::insert_stripe_webhook_event_log::InsertStripeWebhookEventLog;

use crate::stripe::helpers::verify_stripe_webhook_ip_address::verify_stripe_webhook_ip_address;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::charge::charge_succeeded_handler::charge_succeeded_handler;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::checkout_session::checkout_session_completed_handler::checkout_session_completed_handler;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::customer::customer_created_handler::customer_created_handler;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::customer::customer_deleted_handler::customer_deleted_handler;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::customer::customer_updated_handler::customer_updated_handler;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::customer_subscription::customer_subscription_created_handler::customer_subscription_created_handler;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::customer_subscription::customer_subscription_deleted_handler::customer_subscription_deleted_handler;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::customer_subscription::customer_subscription_updated_handler::customer_subscription_updated_handler;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::invoice::invoice_paid_handler::invoice_paid_handler;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::invoice::invoice_payment_failed::invoice_payment_failed_handler;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::invoice::invoice_payment_succeeded_handler::invoice_payment_succeeded_handler;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::invoice::invoice_updated_handler::invoice_updated_handler;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::payment_intent::payment_intent_succeeded_handler::payment_intent_succeeded_handler;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::stripe_webhook_error::StripeWebhookError;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::stripe_webhook_summary::StripeWebhookSummary;
use crate::stripe::stripe_config::StripeConfig;
use crate::stripe::traits::internal_subscription_product_lookup::InternalSubscriptionProductLookup;

#[derive(Serialize)]
pub struct StripeWebhookSuccessResponse {
  pub success: bool,
}

pub async fn stripe_webhook_handler(
  http_request: HttpRequest,
  request_body_bytes: Bytes,
  mysql_pool: web::Data<MySqlPool>,
  stripe_config: web::Data<StripeConfig>,
  internal_subscription_product_lookup: web::Data<dyn InternalSubscriptionProductLookup>,
) -> Result<HttpResponse, StripeWebhookError>
{
  verify_stripe_webhook_ip_address(&http_request)
      .map_err(|e| {
        let reason = format!("Improper client IP address. Error: {:?}", e);
        error!("{}", &reason);
        StripeWebhookError::BadRequest(reason)
      })?;

  let secret_signing_key = &stripe_config.secrets.secret_webhook_signing_key;

  let stripe_signature = get_request_header_optional(&http_request, "Stripe-Signature")
      .unwrap_or_default();

  // NB: Treat the request payload as unstructured and defer to Stripe libraries.
  let webhook_payload = String::from_utf8(request_body_bytes.to_vec())
      .map_err(|err| {
        let reason = format!("Could not decode request body to UTF-8: {:?}", err);
        error!("{}", &reason);
        StripeWebhookError::BadRequest(reason)
      })?;

  let webhook_payload = Webhook::construct_event(&webhook_payload, &stripe_signature, secret_signing_key)
      .map_err(|e| {
        let reason = format!("Could not construct Stripe webhook event: {:?}", e);
        error!("{}", &reason);
        StripeWebhookError::BadRequest(reason)
      })?;

  // Events can be re-sent, so we need to make handling them idempotent.
  let stripe_event_id = webhook_payload.id.to_string();

  let stripe_event_created_at = NaiveDateTime::from_timestamp(webhook_payload.created, 0);

  let stripe_event_type = serde_json::to_string(&webhook_payload.type_)
      .map(|s| s.replace("\"", ""))
      .map_err(|err| {
        let reason = format!("Could not serialize webhook type: {:?}", err);
        error!("{}", &reason);
        StripeWebhookError::BadRequest(reason)
      })?;

  // NB: Whether this was test data or live data
  let stripe_is_production = webhook_payload.livemode;

  info!("Stripe webhook event type: {} ({:?}); is production: {}, created at: {}, pending events to be handled: {}",
    &stripe_event_type,
    &webhook_payload.type_,
    stripe_is_production,
    &stripe_event_created_at,
    webhook_payload.pending_webhooks);

  let maybe_previously_played_event = get_stripe_webhook_event_log_by_id(&stripe_event_id, &mysql_pool)
      .await
      .map_err(|err| {
        let reason = format!("Could not query previous event by ID ({}): {:?}", &stripe_event_id, err);
        error!("{}", &reason);
        StripeWebhookError::ServerError(reason)
      })?;

  if let Some(event) = maybe_previously_played_event {
    // The event is being replayed by Stripe, and we've already handled it.
    // We'll ignore it so that we remain idempotent.
    if event.should_ignore_retry {
      warn!("Stripe is replying event with ID={}. Ignoring it since we have previously handled it.",
        &stripe_event_id);
      return report_success();
    }
  }

  let mut unhandled_event_type = false;

  let mut webhook_summary = StripeWebhookSummary {
    maybe_user_token: None,
    maybe_event_entity_id: None,
    maybe_stripe_customer_id: None,
    action_was_taken: false,
    should_ignore_retry: false,
  };

  // NB:
  // - "Webhook endpoints might occasionally receive the same event more than once."
  // - "Stripe does not guarantee delivery of events in the order in which they are generated."
  match webhook_payload.type_ {

    // =============== CHECKOUT SESSIONS ===============

    EventType::CheckoutSessionCompleted => {
      if let EventObject::CheckoutSession(checkout_session) = webhook_payload.data.object {
        webhook_summary = checkout_session_completed_handler(checkout_session)?;
      }
    }

    // EventType::CheckoutSessionExpired => {}
    // EventType::CheckoutSessionAsyncPaymentFailed => {}
    // EventType::CheckoutSessionAsyncPaymentSucceeded => {}

    // =============== CUSTOMERS ===============

    EventType::CustomerCreated => {
      if let EventObject::Customer(customer) = webhook_payload.data.object {
        webhook_summary = customer_created_handler(&customer)?;
      }
    }
    EventType::CustomerUpdated => {
      if let EventObject::Customer(customer) = webhook_payload.data.object {
        webhook_summary = customer_updated_handler(&customer)?;
      }
    }
    EventType::CustomerDeleted => {
      if let EventObject::Customer(customer) = webhook_payload.data.object {
        webhook_summary = customer_deleted_handler(&customer)?;
      }
    }

    // =============== CUSTOMER SUBSCRIPTIONS ===============

    EventType::CustomerSubscriptionCreated => {
      if let EventObject::Subscription(subscription) = webhook_payload.data.object {
        webhook_summary = customer_subscription_created_handler(
          &subscription,
          internal_subscription_product_lookup.get_ref(),
          &mysql_pool).await?;
      }
    }
    EventType::CustomerSubscriptionUpdated => {
      if let EventObject::Subscription(subscription) = webhook_payload.data.object {
        webhook_summary = customer_subscription_updated_handler(
          &subscription,
          internal_subscription_product_lookup.get_ref(),
          &mysql_pool).await?;
      }
    }
    EventType::CustomerSubscriptionDeleted => {
      if let EventObject::Subscription(subscription) = webhook_payload.data.object {
        webhook_summary = customer_subscription_deleted_handler(
          &subscription,
          internal_subscription_product_lookup.get_ref(),
          &mysql_pool).await?;
      }
    }

    // EventType::CustomerSubscriptionPendingUpdateApplied => {}
    // EventType::CustomerSubscriptionPendingUpdateExpired => {}
    // EventType::CustomerSubscriptionTrialWillEnd => {}

    // =============== INVOICES ===============

    EventType::InvoiceCreated => {
      // TODO: We need to respond to this so we don't hold payments up by 72 hours!
      //  See: https://stripe.com/docs/billing/subscriptions/webhooks
    }
    EventType::InvoiceUpdated => {
      if let EventObject::Invoice(invoice) = webhook_payload.data.object {
        webhook_summary = invoice_updated_handler(&invoice)?;
      }
    }
    EventType::InvoicePaid => {
      if let EventObject::Invoice(invoice) = webhook_payload.data.object {
        webhook_summary = invoice_paid_handler(&invoice)?;
      }
    }
    EventType::InvoicePaymentSucceeded => {
      if let EventObject::Invoice(invoice) = webhook_payload.data.object {
        webhook_summary = invoice_payment_succeeded_handler(&invoice)?;
      }
    }
    EventType::InvoicePaymentFailed => {
      if let EventObject::Invoice(invoice) = webhook_payload.data.object {
        webhook_summary = invoice_payment_failed_handler(&invoice)?;
      }
    }

    // EventType::InvoiceDeleted => {}
    // EventType::InvoiceFinalizationFailed => {}
    // EventType::InvoiceFinalized => {}
    // EventType::InvoiceItemCreated => {}
    // EventType::InvoiceItemDeleted => {}
    // EventType::InvoiceItemUpdated => {}
    // EventType::InvoiceMarkedUncollectible => {}
    // EventType::InvoicePaymentActionRequired => {}
    // EventType::InvoiceSent => {}
    // EventType::InvoiceUpcoming => {}
    // EventType::InvoiceUpdated => {}
    // EventType::InvoiceVoided => {}

    // =============== PAYMENT INTENTS ===============

    EventType::PaymentIntentSucceeded => {
      if let EventObject::PaymentIntent(payment_intent) = webhook_payload.data.object {
        webhook_summary = payment_intent_succeeded_handler(&payment_intent)?;
      }
    }

    // EventType::PaymentIntentAmountCapturableUpdated => {}
    // EventType::PaymentIntentCanceled => {}
    // EventType::PaymentIntentCreated => {}
    // EventType::PaymentIntentPartiallyFunded => {}
    // EventType::PaymentIntentPaymentFailed => {}
    // EventType::PaymentIntentProcessing => {}
    // EventType::PaymentIntentRequiresAction => {}
    // EventType::PaymentIntentRequiresCapture => {}

    // =============== CHARGES ===============

    EventType::ChargeSucceeded => {
      if let EventObject::Charge(charge) = webhook_payload.data.object {
        webhook_summary = charge_succeeded_handler(&charge)?;
      }
    }


    // =============== Ignored ===============

    // Ignoring these types:
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

    _ => {
      unhandled_event_type = true;
    },
  }

  // To play with the payload contents as JSON:
  // let json = serde_json::ser::to_string(&event_payload).unwrap();
  // info!("event payload as json: {}", json);

  if unhandled_event_type {
    warn!("Unhandled Stripe webhook event type: {} ({:?})",
      &stripe_event_type,
      &webhook_payload.type_);
  }

  let query = InsertStripeWebhookEventLog {
    stripe_event_id,
    stripe_event_type,
    maybe_stripe_event_entity_id: webhook_summary.maybe_event_entity_id,
    maybe_stripe_customer_id: webhook_summary.maybe_stripe_customer_id,
    stripe_event_created_at,
    stripe_is_production,
    maybe_user_token: webhook_summary.maybe_user_token,
    action_was_taken: webhook_summary.action_was_taken,
    should_ignore_retry: webhook_summary.should_ignore_retry,
  };

  query.insert(&mysql_pool)
      .await
      .map_err(|err| {
        let reason = format!("Could not insert Stripe webhook event log: {:?}", err);
        error!("{}", &reason);
        StripeWebhookError::ServerError(reason)
      })?;

  report_success()
}

#[inline]
fn report_success() -> Result<HttpResponse, StripeWebhookError> {
  let response = StripeWebhookSuccessResponse {
    success: true,
  };

  let body = serde_json::to_string(&response)
      .map_err(|_e| StripeWebhookError::ServerError(_e.to_string()))?;

  Ok(HttpResponse::Ok()
      .content_type("application/json")
      .body(body))
}
