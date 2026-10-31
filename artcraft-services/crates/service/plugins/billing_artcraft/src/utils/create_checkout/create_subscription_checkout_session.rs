use crate::configs::stripe_artcraft_metadata_keys::{STRIPE_ARTCRAFT_METADATA_EMAIL, STRIPE_ARTCRAFT_METADATA_USERNAME, STRIPE_ARTCRAFT_METADATA_USER_TOKEN};
use crate::utils::common_web_error::CommonWebError;
use log::{error, info};
use std::collections::HashMap;
use stripe::Client;
use stripe_checkout::checkout_session::{CreateCheckoutSession, CreateCheckoutSessionAutomaticTax, CreateCheckoutSessionLineItems, CreateCheckoutSessionSavedPaymentMethodOptions, CreateCheckoutSessionSavedPaymentMethodOptionsAllowRedisplayFilters, CreateCheckoutSessionSavedPaymentMethodOptionsPaymentMethodSave, CreateCheckoutSessionSubscriptionData};
use stripe_shared::{CheckoutSession, CheckoutSessionMode, CustomerId, PriceId};
use tokens::tokens::users::UserToken;

pub struct CreateSubscriptionCheckoutSessionArgs<'a> {
  /// Subscription is required
  pub subscription_price_id: &'a PriceId,
  
  /// Optional: Existing user's Stripe customer id, if it exists.
  /// You MUST look this up.
  pub maybe_existing_stripe_customer_id: Option<&'a CustomerId>,

  /// User is required
  pub user_token: &'a UserToken,

  /// Username is optional in the case we eagerly created a user account for the user.
  pub username: Option<&'a str>,
  
  /// User email is optional in the case we eagerly created a user account for the user.
  pub user_email: Option<&'a str>,
  
  pub stripe_client: &'a Client, 
  
  pub success_url: &'a str,
  pub cancel_url: &'a str,
}

pub async fn create_subscription_checkout_session(args: CreateSubscriptionCheckoutSessionArgs<'_>) -> Result<CheckoutSession, CommonWebError> {

  let checkout_session = {
    // `client_reference_id`
    // Stripe Docs:
    //   A unique string to reference the Checkout Session.
    //   This can be a customer ID, a cart ID, or similar, and can be used to reconcile the session
    //   with your internal systems.
    //
    // Our Notes:
    //   This gets reported back in the Checkout Session (and related webhooks) as
    //   `client_reference_id`. Passing the same ID on multiple checkouts does not unify or
    //   cross-correlate customers and only seems to be metadata for the checkout session itself.
    //   This is probably only useful for tracking checkout session engagement.
    //params.client_reference_id = Some("SOME_INTERNAL_ID");

    // `customer_email`
    // Stripe Docs:
    //   If provided, this value will be used when the Customer object is created. If not provided,
    //   customers will be asked to enter their email address. Use this parameter to prefill
    //   customer data if you already have an email on file. To access information about the
    //   customer once a session is complete, use the customer field.
    //
    // Our Notes:
    //   This does not look up previous customers with the same email and will not unify or
    //   cross-correlate customers. By default the field will be un-editable in the checkout flow
    //   if this is specified.
    //params.customer_email = Some("email@example.com");

    let mut metadata = HashMap::new();

    metadata.insert(STRIPE_ARTCRAFT_METADATA_USER_TOKEN.to_string(), args.user_token.to_string());

    if let Some(username) = args.username.as_deref() {
      metadata.insert(STRIPE_ARTCRAFT_METADATA_USERNAME.to_string(), username.to_string());
    }

    if let Some(user_email) = args.user_email.as_deref() {
      metadata.insert(STRIPE_ARTCRAFT_METADATA_EMAIL.to_string(), user_email.to_string());
    }

    let mut checkout_builder = CreateCheckoutSession::new()
        .success_url(args.success_url)
        .cancel_url(args.cancel_url)
        .mode(CheckoutSessionMode::Subscription)
        .line_items(vec![
          CreateCheckoutSessionLineItems {
            price: Some(args.subscription_price_id.to_string()),
            quantity: Some(1),
            ..Default::default()
          }
        ])
        .saved_payment_method_options(CreateCheckoutSessionSavedPaymentMethodOptions {
          allow_redisplay_filters: Some(vec![
            CreateCheckoutSessionSavedPaymentMethodOptionsAllowRedisplayFilters::Always,
            CreateCheckoutSessionSavedPaymentMethodOptionsAllowRedisplayFilters::Limited,
            CreateCheckoutSessionSavedPaymentMethodOptionsAllowRedisplayFilters::Unspecified,
          ]),
          // The user can choose to tick a checkbox that saves their card for redisplay later.
          payment_method_save: Some(CreateCheckoutSessionSavedPaymentMethodOptionsPaymentMethodSave::Enabled),
        })
        .allow_promotion_codes(true) // Allow promo codes / coupons
        .automatic_tax(CreateCheckoutSessionAutomaticTax {
          enabled: true, // This will ask for the customer's location
          liability: None,
        })
        .metadata(metadata.clone())
        .subscription_data(CreateCheckoutSessionSubscriptionData {
          metadata: Some(metadata),
          ..Default::default()
        })
        ;

    if let Some(customer_id) = args.maybe_existing_stripe_customer_id {
      info!("Adding existing stripe customer id to checkout session: {}", customer_id.as_str());
      checkout_builder = checkout_builder.customer(customer_id);
    }

    let checkout_session = checkout_builder
        .send(args.stripe_client)
        .await
        .map_err(|err| {
          error!("Stripe Error: {:?}", err);
          CommonWebError::ServerError
        })?;

    checkout_session
  };
  
  Ok(checkout_session)
}