-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- 1:1 links between a user and a stripe customer (per payments namespace)
-- We can hard delete records if we have to.
-- If users need a new stripe customer ID, we need to hard delete the existing record.
CREATE TABLE user_stripe_customer_links (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Composite primary key
  user_token VARCHAR(32) NOT NULL,

  -- Composite primary key
  -- If we host multiple sites with distinct categories, this will enable us to
  -- segregate accounts.
  -- NB: Uses the `PaymentsNamespace` enum, which is also shared with subscriptions.
  payments_namespace VARCHAR(32) NOT NULL,

  -- Unique key. (We can loosen to `stripe_customer_id, wallet_namespace` later if
  -- we want to share billing.)
  stripe_customer_id VARCHAR(255) NOT NULL,

  -- ========== RECORD TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- ========== INDICES ==========
  PRIMARY KEY (id),
  UNIQUE KEY (user_token, payments_namespace),
  -- UNIQUE KEY (stripe_customer_id, payments_namespace)
  UNIQUE KEY (stripe_customer_id)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
