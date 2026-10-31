-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE stripe_webhook_event_logs (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- ========== STRIPE DATA ==========

  -- You can safely assume object IDs we generate will never exceed 255 characters,
  -- but you should be able to handle IDs of up to that length. If for example you’re using MySQL,
  -- you should store IDs in a VARCHAR(255) COLLATE utf8_bin column (the COLLATE configuration ensures
  -- case-sensitivity in lookups).
  -- https://stackoverflow.com/a/61809494
  stripe_event_id VARCHAR(255) NOT NULL,

  -- The type of webhook event
  stripe_event_type VARCHAR(255) NOT NULL,

  -- The id of the entity that was the subject of the event.
  -- We don't always extract this.
  maybe_stripe_event_entity_id VARCHAR(255) DEFAULT NULL,

  -- The id of the entity that was the subject of the event.
  -- We don't always extract this.
  maybe_stripe_customer_id VARCHAR(255) DEFAULT NULL,

  -- Whether this is in production or test mode in Stripe.
  -- This is controlled by which API keys are used.
  -- "livemode"=true in production.
  stripe_is_production BOOLEAN NOT NULL DEFAULT FALSE,

  -- ========== INTERNAL SYSTEM METADATA ==========

  -- Whether we handled the event with any sort of action
  action_was_taken BOOLEAN NOT NULL DEFAULT FALSE,

  -- Whether we should ignore any future retries by Stripe replaying the event.
  -- This helps us treat events with idempotency.
  should_ignore_retry BOOLEAN NOT NULL DEFAULT FALSE,

  -- Some event types may attach a user token
  maybe_user_token VARCHAR(32) DEFAULT NULL,

  -- An internal optional entity token this resolves to.
  --  eg. `wallets.token` or `user_subscriptions.token`.
  maybe_internal_entity_token VARCHAR(32) DEFAULT NULL,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- When the event was created on Stripe's end
  stripe_event_created_at TIMESTAMP NOT NULL,

  -- ========== INDICES ==========
  PRIMARY KEY (id),
  UNIQUE KEY (stripe_event_id),
  KEY index_stripe_event_type (stripe_event_type),
  KEY index_maybe_stripe_event_entity_id (maybe_stripe_event_entity_id),
  KEY index_maybe_stripe_customer_id (maybe_stripe_customer_id),
  KEY fk_maybe_user_token (maybe_user_token)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
