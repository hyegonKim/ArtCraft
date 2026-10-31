-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE wallets (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- If we host multiple sites with distinct categories, this will enable us to
  -- segregate wallets. This is effectively a wallet namespace.
  -- NB: Uses the `PaymentsNamespace` enum, which is also shared with subscriptions.
  wallet_namespace VARCHAR(32) NOT NULL,

  -- Who owns the wallet
  -- In the future, we might allow other users to use the same wallet,
  -- but the wallet would still have an owner.
  -- For now, we'll keep this 1:1 with users, but we may want 1:n in the future.
  owner_user_token VARCHAR(32) NOT NULL,

  -- The durable credits bucket.
  -- This bucket is persistent and is refilled by user credit purchases.
  -- Max value unsigned: 4,294,967,295 (U32)
  banked_credits INTEGER UNSIGNED NOT NULL DEFAULT 0,

  -- The monthly credits bucket.
  -- This bucket is ephemeral and is refilled monthly.
  -- Max value unsigned: 4,294,967,295 (U32)
  monthly_credits INTEGER UNSIGNED NOT NULL DEFAULT 0,

  -- Whether the stripe subscription is active.
  -- subscription_is_active BOOLEAN NOT NULL DEFAULT FALSE,

  -- Whether the stripe subscription is paid.
  -- If the subscription is not paid, we should not allow usage of monthly credits.
  -- subscription_is_paid BOOLEAN NOT NULL DEFAULT FALSE,

  -- ========== VECTOR CLOCK ==========

  -- Incremented with every update.
  version INT NOT NULL DEFAULT 0,

  -- ========== RECORD TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- ========== PAYMENTS AND SUBSCRIPTIONS TIMESTAMPS ==========

  -- When the user last purchased banked (durable) credits.
  -- banked_credits_last_purchased_at TIMESTAMP DEFAULT NULL,

  -- When we last refilled the monthly bucket.
  -- This has to do with subscription logic.
  -- monthly_wallet_refilled_at TIMESTAMP DEFAULT NULL,

  -- ========== INDICES ==========
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  UNIQUE KEY (wallet_namespace, owner_user_token), -- For now
  INDEX idx_wallet_namespace (wallet_namespace),
  INDEX idx_owner_user_token (owner_user_token)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
