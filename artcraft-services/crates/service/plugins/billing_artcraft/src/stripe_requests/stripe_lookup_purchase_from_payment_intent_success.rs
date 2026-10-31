use crate::configs::stripe_client_retry_strategy::STRIPE_CLIENT_RETRY_STRATEGY;
use crate::utils::expand_ids::expand_product_id::expand_product_id;
use log::{error, warn};
use stripe::{Client, StripeRequest};
use stripe_checkout::checkout_session::{ListCheckoutSession, ListLineItemsCheckoutSession};
use stripe_shared::{CheckoutSession, CheckoutSessionItem, PriceType};
use stripe_types::List;

#[derive(Debug, Clone)]
pub struct PurchaseSummary {
  pub price_id: String,
  pub product_id: String,
  pub quantity: u64,

  /// Subscriptions have invoices, including the first payment. One-off purchases do not.
  pub has_invoice: bool,
}

// This assumes the payment came from a checkout session.
pub async fn stripe_lookup_purchase_from_payment_intent_success(
  payment_intent_id: &str,
  stripe_client: &Client
) -> anyhow::Result<PurchaseSummary> {

  let result = ListCheckoutSession::new()
      .payment_intent(payment_intent_id)
      .limit(1)
      .build()
      .customize::<List<CheckoutSession>>()
      .request_strategy(STRIPE_CLIENT_RETRY_STRATEGY)
      .send(stripe_client)
      .await
      .map_err(|err| {
        error!("Stripe Error looking up checkout session for payment intent {}: {:?}", &payment_intent_id, err);
        err
      })?;

  if result.data.len() != 1 {
    error!("Expected exactly one checkout session for payment intent {}, found {}", payment_intent_id, result.data.len());
    return Err(anyhow::anyhow!("Expected exactly one checkout session for payment intent {}, found {}", payment_intent_id, result.data.len()));
  }

  let checkout_session = result
      .data
      .get(0)
      .ok_or_else(|| anyhow::anyhow!("No items in ListCheckoutSession request."))?;

  let has_invoice = checkout_session.invoice.is_some();

  let checkout_session_id = result
      .data
      .get(0)
      .map(|session| &session.id)
      .ok_or_else(|| anyhow::anyhow!("No items in ListCheckoutSession request."))?;

  let line_items = ListLineItemsCheckoutSession::new(checkout_session_id)
      .build()
      .customize::<List<CheckoutSessionItem>>()
      .request_strategy(STRIPE_CLIENT_RETRY_STRATEGY)
      .send(stripe_client)
      .await
      .map_err(|err| {
        error!("Stripe Error looking up checkout session for payment intent {}: {:?}", &payment_intent_id, err);
        err
      })?;

  if line_items.data.len() != 1 {
    warn!("Unexpected number of line items: {} for checkout session {}", line_items.data.len(), checkout_session_id);
  }

  let line_item = line_items
      .data
      .get(0)
      .ok_or_else(|| anyhow::anyhow!("No line items found in ListCheckoutSession request."))?;

  let price = line_item
      .price
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("No items in ListCheckoutSession request."))?;

  match price.type_ {
    PriceType::OneTime => {}
    PriceType::Recurring => {}
  }

  let price_id = price.id.to_string();
  let product_id = expand_product_id(&price.product);
  let quantity = line_item.quantity.unwrap_or(1);

  Ok(PurchaseSummary {
    price_id,
    product_id,
    quantity,
    has_invoice,
  })
}
