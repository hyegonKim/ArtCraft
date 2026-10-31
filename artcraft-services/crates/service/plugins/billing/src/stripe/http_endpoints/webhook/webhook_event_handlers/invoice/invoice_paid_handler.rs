use chrono::{DateTime, Utc};
use log::error;
use stripe::{Invoice, InvoiceLineItemType, InvoiceStatus, SubscriptionInterval};

use errors::AnyhowResult;

use crate::stripe::helpers::common_metadata_keys::METADATA_USER_TOKEN;
use crate::stripe::helpers::expand_customer_id::expand_customer_id;
use crate::stripe::helpers::expand_product_id::expand_product_id;
use crate::stripe::helpers::expand_subscription_id::expand_subscription_id;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::stripe_webhook_error::StripeWebhookError;
use crate::stripe::http_endpoints::webhook::webhook_event_handlers::stripe_webhook_summary::StripeWebhookSummary;

/// If we determine a subscription needs update, this is the struct we produce.
/// This should be easily unit testable.
pub struct SubscriptionDetails {
  // Internal user token (not Stripe)
  pub user_token: Option<String>,

  pub stripe_is_production: bool,

  pub stripe_invoice_id: String,

  pub stripe_customer_id: Option<String>,
  pub stripe_subscription_id: Option<String>,
  pub stripe_product_id: Option<String>,
  pub stripe_price_id: Option<String>,

  pub subscription_is_active: bool,

  pub billed_at: DateTime<Utc>,

  /// Tell the update handler how many days in the future to set the plan.
  pub subscription_interval: SubscriptionInterval,

  /// Calculated from interval
  pub subscription_expires_at: DateTime<Utc>,
}

/// If we determine a product was paid for, this is the struct we produce.
/// This should be easily unit testable.
pub struct ProductDetails {
  pub user_token: Option<String>,

  pub stripe_customer_id: Option<String>,
  pub stripe_product_id: Option<String>,
}

/// For now we only handle one singular item at a time.
/// (Technically this can handle many products and subscriptions at once.)
pub enum InvoicePaidDetails {
  Subscription(SubscriptionDetails),
  Product(ProductDetails),
}

// Handle event type: 'invoice.paid'
// // https://stripe.com/docs/billing/subscriptions/webhooks :
//
// Sent when the invoice is successfully paid. You can provision access to your product when you
// receive this event and the subscription `status` is `active`.
//
// https://stripe.com/docs/billing/subscriptions/webhooks#active-subscriptions :
//
// 1. A few days prior to renewal, your site receives an invoice.upcoming event at the webhook
//    endpoint. You can listen for this event to add extra invoice items to the upcoming invoice.
// 2. Your site receives an invoice.paid event.
// 3. Your webhook endpoint finds the customer the payment was made for.
// 4. Your webhook endpoint updates the customer’s access expiration date in your database to the
//    appropriate date in the future (plus a day or two for leeway).
//
pub fn invoice_paid_handler(invoice: &Invoice) -> Result<StripeWebhookSummary, StripeWebhookError> {
  let maybe_invoice_details = invoice_paid_extractor(invoice)
      .map_err(|err| {
        let reason = format!("Error extracting invoice details from 'invoice.paid' payload: {:?}", err);
        error!("{}", reason);
        StripeWebhookError::ServerError(reason)
      })?;

  // TODO
  Ok(StripeWebhookSummary {
    maybe_user_token: None,
    maybe_event_entity_id: None,
    maybe_stripe_customer_id: None,
    action_was_taken: false,
    should_ignore_retry: false,
  })
}

fn invoice_paid_extractor(invoice: &Invoice) -> AnyhowResult<Option<InvoicePaidDetails>> {
  match invoice.status {
    Some(InvoiceStatus::Paid) => {}
    _ => {
      return Ok(None);
    },
  }

  let invoice_id = invoice.id.to_string();
  let is_production= invoice.livemode.unwrap_or(false);
  let paid_status = invoice.status;

  let maybe_stripe_customer_id  = invoice.customer
      .as_ref()
      .map(|c| expand_customer_id(c));

  let maybe_stripe_subscription_id = invoice.subscription
      .as_ref()
      .map(|s| expand_subscription_id(s));

  // TODO: We only handle a single line item for now.
  let line_item = match invoice.lines.data.first() {
    None => return Ok(None),
    Some(line_item) => line_item,
  };

  let paid_details = match line_item.type_ {
    InvoiceLineItemType::InvoiceItem => {
      return Ok(None); // TODO: Handle one-time payments.
    }
    InvoiceLineItemType::Subscription => {
      let maybe_subscription_id = line_item.subscription
          .as_ref().map(|s| expand_subscription_id(s));

      let maybe_product_id = line_item.price
          .as_ref()
          .and_then(|price| price.product.as_ref())
          .map(|product| expand_product_id(product));

      let maybe_price_id = line_item.price
          .as_ref()
          .map(|price| price.id.to_string());

      // NB: Internal user token (non-stripe)
      let maybe_user_token = line_item.metadata.get(METADATA_USER_TOKEN)
          .map(|d| d.to_string());

      InvoicePaidDetails::Subscription(SubscriptionDetails {
        user_token: maybe_user_token,
        stripe_is_production: is_production,
        stripe_invoice_id: invoice_id,
        stripe_customer_id: maybe_stripe_customer_id,
        stripe_subscription_id: maybe_subscription_id,
        stripe_product_id: maybe_product_id,
        stripe_price_id: maybe_price_id,
        subscription_is_active: false,
        billed_at: Utc::now(),
        subscription_interval: Default::default(),
        subscription_expires_at: Utc::now(),
      })
    }
  };

  Ok(Some(paid_details))
}


#[cfg(test)]
mod tests {
  use anyhow::bail;
  use stripe::Invoice;
  use errors::AnyhowResult;

  use crate::stripe::http_endpoints::webhook::webhook_event_handlers::invoice::invoice_paid_handler::invoice_paid_extractor;
  use crate::stripe::http_endpoints::webhook::webhook_event_handlers::invoice::invoice_paid_handler::InvoicePaidDetails;

  #[test]
  fn test_invoice_paid_extractor() -> AnyhowResult<()> {
    // NB: Not the prettiest, but this is a dev mode capture of a subscription "invoice.paid" event:
    let invoice = r#"
      {"id":"in_1Lh1R6EU5se17Mekksuvq20H","account_country":"US","account_name":"Storyteller","amount_due":700,"amount_paid":700,"amount_remaining":0,
      "attempt_count":1,"attempted":true,"auto_advance":false,"automatic_tax":{"enabled":false},"billing_reason":"subscription_create","charge":"ch_3Lh1R7EU5se17Mek1StoPTgz",
      "collection_method":"charge_automatically","created":1662946064,"currency":"usd","customer":"cus_MPr9UxT6b4SR5v","customer_address":{"country":"US",
      "postal_code":"44444"},"customer_email":"foo@bar.com","customer_name":"Name","customer_tax_exempt":"none","customer_tax_ids":[],
      "default_tax_rates":[],"deleted":false,"discounts":[],"ending_balance":0,
      "hosted_invoice_url":"https://invoice.stripe.com/i/acct_1KZvWXEU5se17Mek/test_YWNjdF8xS1p2V1hFVTVzZTE3TWVrLF9NUHI5YjVkM0FrWExVQTVUQ1J1bFNaRWhqcHFlTU9VLDUzNDg2ODY402005IZTB0v6?s=ap",
      "invoice_pdf":"https://pay.stripe.com/invoice/acct_1KZvWXEU5se17Mek/test_YWNjdF8xS1p2V1hFVTVzZTE3TWVrLF9NUHI5YjVkM0FrWExVQTVUQ1J1bFNaRWhqcHFlTU9VLDUzNDg2ODY402005IZTB0v6/pdf?s=ap",
      "lines":{"data":[{"id":"il_1Lh1R6EU5se17MekXq9iJCZi",
      "amount":700,"currency":"usd","description":"1 × FakeYou basic (at $7.00 / month)","discount_amounts":[],"discountable":true,"discounts":[],"livemode":false,
      "metadata":{"username":"_username_","user_token":"U:TOKEN","email":"user@name.com"},"period":{"end":1665538064,"start":1662946064},"price":{"id":"price_1LeDnKEU5se17MekVr1iYYNf",
      "active":true,"billing_scheme":"per_unit","created":1662278586,"currency":"usd","deleted":false,"livemode":false,"metadata":{},"product":"prod_MMxi2J5y69VPbO",
      "recurring":{"interval":"month","interval_count":1,"usage_type":"licensed"},"tax_behavior":"exclusive","type":"recurring","unit_amount":700,"unit_amount_decimal":"700"},
      "proration":false,"proration_details":{},"quantity":1,"subscription":"sub_1Lh1R6EU5se17Mekq9Ahj0gi",
      "subscription_item":"si_MPr9vVB80UAGwp","tax_amounts":[],"tax_rates":[],"type":"subscription"}],"has_more":false,"total_count":1,
      "url":"/v1/invoices/in_1Lh1R6EU5se17Mekksuvq20H/lines"},
      "livemode":false,"metadata":{},"number":"4F52B2F7-0001","paid":true,"paid_out_of_band":false,"payment_intent":"pi_3Lh1R7EU5se17Mek1Bby1wL8",
      "payment_settings":{},"period_end":1662946064,"period_start":1662946064,"post_payment_credit_notes_amount":0,
      "pre_payment_credit_notes_amount":0,"starting_balance":0,"status":"paid",
      "status_transitions":{"finalized_at":1662946064,"paid_at":1662946066},"subscription":"sub_1Lh1R6EU5se17Mekq9Ahj0gi",
      "subtotal":700,"total":700,"total_discount_amounts":[],"total_tax_amounts":[]}
    "#;

    let invoice = serde_json::from_str::<Invoice>(invoice).unwrap();

    let details = invoice_paid_extractor(&invoice).unwrap().unwrap();

    match details {
      InvoicePaidDetails::Product(_) => { bail!("This is supposed to be a subscription!"); }
      InvoicePaidDetails::Subscription(subscription) => {
        assert_eq!(subscription.stripe_invoice_id, "in_1Lh1R6EU5se17Mekksuvq20H".to_string());
        assert_eq!(subscription.user_token, Some("U:TOKEN".to_string()));
      }
    }

    Ok(())
  }
}