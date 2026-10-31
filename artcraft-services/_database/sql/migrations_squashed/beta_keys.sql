-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE beta_keys (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- ========== PRIMARY, FOREIGN KEYS, and POLYMORPHISM ==========

  -- Effective "primary key" of the beta key.
  token VARCHAR(32) NOT NULL,

  -- ========== KEY INFO ==========

  -- Name of the product the key applies to
  product VARCHAR(32) NOT NULL,

  -- The actual key value that the user inputs
  -- Crockford-encoded.
  key_value VARCHAR(32) NOT NULL,

  -- ========== CREATOR USER ==========

  -- This is a member on our team
  creator_user_token VARCHAR(32) NOT NULL,

  -- ========== REFERRER USER ==========

  -- The user that offers the beta key to other users.
  -- We can show a dashboard of unredeemed tokens for these users.
  maybe_referrer_user_token VARCHAR(32) DEFAULT NULL,

  -- ========== REDEEMER USER ==========

  -- The user that redeems the beta key.
  -- If set, the key has been redeemed.
  maybe_redeemer_user_token VARCHAR(32) DEFAULT NULL,

  -- The user that redeems the beta key.
  -- Based on a cookie sent by the frontend.
  maybe_redeemer_anonymous_visitor_token VARCHAR(32) DEFAULT NULL,

  -- The user that redeems the beta key.
  -- Wide enough for IPv4/6
  maybe_redeemer_ip_address VARCHAR(40) DEFAULT NULL,

  -- ========== EXTRA INFO ==========

  -- A flag to manually indicate if the key was given away to a user.
  -- This might be useful when generating flags in bulk.
  is_distributed BOOLEAN NOT NULL DEFAULT FALSE,

  -- Optional notes that can be attached to a beta key
  maybe_notes VARCHAR(255) DEFAULT NULL,

  -- ========== TIMESTAMPS ==========

  -- When the key was created
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

  -- When the key expires (optional)
  maybe_expires_at TIMESTAMP NULL,

  -- When the key was redeemed
  maybe_redeemed_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  UNIQUE KEY (key_value),

  KEY index_creator_user_token (creator_user_token),
  KEY index_maybe_referrer_user_token (maybe_referrer_user_token),
  KEY index_maybe_redeemer_user_token (maybe_redeemer_user_token),
  KEY index_created_at (created_at),
  KEY index_maybe_redeemed_at (maybe_redeemed_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
