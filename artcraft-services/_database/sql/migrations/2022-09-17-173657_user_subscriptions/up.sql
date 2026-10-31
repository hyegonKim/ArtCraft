-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE user_subscriptions (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- ========== INTERNAL SUBSCRIPTION DETAILS ==========

  -- The internal user associated with the subscription.
  -- This is nullable since it's possible we mess up and don't set it on the Stripe objects.
  -- We want the latitude to correct mistakes in the future. (Hopefully this doesn't bite me later.)
  -- **** NB/NOTE: THIS COLUMN WAS RENAMED TO `user_token` and made NON NULLABLE. ****
  maybe_user_token VARCHAR(32) DEFAULT NULL,

  -- If we host multiple sites with distinct categories, this will enable us to segregate
  -- subscriptions. This is effectively a subscriptions namespace.
  -- **** NB/NOTE: THIS COLUMN WAS RENAMED TO `subscription_namespace` ****
  subscription_category VARCHAR(32) NOT NULL,

  -- This is the identifier for the actual type of subscription.
  -- These will not be defined in the database, but rather the source code.
  -- **** NB/NOTE: THIS COLUMN WAS RENAMED TO `subscription_product_slug` ****
  subscription_product_key VARCHAR(32) NOT NULL,

  -- ========== STRIPE DATA ==========

  -- The stripe IDs (nullable in case we can't associate or if we use paypal)

  -- You can safely assume object IDs we generate will never exceed 255 characters,
  -- but you should be able to handle IDs of up to that length. If for example you’re using MySQL,
  -- you should store IDs in a VARCHAR(255) COLLATE utf8_bin column (the COLLATE configuration ensures
  -- case-sensitivity in lookups).
  -- https://stackoverflow.com/a/61809494

  -- When we receive a webhook update, we'll look it up by this key.
  -- This has a UNIQUE index.
  maybe_stripe_subscription_id VARCHAR(255) DEFAULT NULL,

  maybe_stripe_customer_id VARCHAR(255) DEFAULT NULL,

  maybe_stripe_product_id VARCHAR(255) DEFAULT NULL,

  maybe_stripe_price_id VARCHAR(255) DEFAULT NULL,

  -- How frequently the subscription is billed and updated
  maybe_stripe_recurring_interval VARCHAR(32) DEFAULT NULL,

  -- Subscription object status enum as string
  maybe_stripe_subscription_status VARCHAR(32) DEFAULT NULL,

  maybe_stripe_is_production BOOLEAN DEFAULT NULL,

  -- ========== VECTOR CLOCK ==========

  -- Incremented with every update.
  version INT NOT NULL DEFAULT 0,

  -- ========== RECORD TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  deleted_at TIMESTAMP DEFAULT NULL,

  -- ========== SUBSCRIPTION TIMESTAMPS ==========

  -- When the subscription was first created (Stripe's subscription "start_date").
  -- This may predate the Stripe object `created` timestamp due to backdating.
  subscription_start_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

  -- We'll use this to determine if the subscription is active.
  --
  -- Always compare against MySQL's clock rather than the app's clock so that we
  -- don't get weird clock skew behaviors across multiple requests.
  --
  -- Technically the user can have multiple active subscriptions of the same type, but we'll
  -- try to prevent creation of new subscriptions of the same type.
  subscription_expires_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

  -- Billing periods.
  -- Maybe useful for debugging.
  current_billing_period_start_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  current_billing_period_end_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

  -- Subscription cancellation (future and past)
  maybe_cancel_at TIMESTAMP DEFAULT NULL,
  maybe_canceled_at TIMESTAMP DEFAULT NULL,

    -- ========== INDICES ==========
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  UNIQUE KEY (maybe_stripe_subscription_id),
  KEY index_subscription_category (subscription_category),
  KEY index_subscription_product_key (subscription_product_key),
  KEY index_maybe_stripe_customer_id (maybe_stripe_customer_id),
  KEY index_maybe_stripe_product_id (maybe_stripe_product_id),
  KEY index_maybe_stripe_subscription_status (maybe_stripe_subscription_status),
  KEY fk_maybe_user_token (maybe_user_token),
  KEY index_subscription_expires_at (subscription_expires_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
