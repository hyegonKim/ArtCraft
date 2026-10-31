-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Per-user password resets.
-- Password resets are valid until any of the following are true:
--
--   1) database NOW() > password_rests.expires_at
--   2) password_rests.is_redeemed == TRUE
--   3) users.password_version > password_resets.current_password_version
--
-- When creating a new password reset, send the user an email with the `secret_key` in the URL.
-- The user will have to supply their username and/or email address, plus the secret key to
-- redeem the password reset.
--
-- Also when creating the password hash, be sure to set `expires_at` to be "NOW() + 3 hours", or
-- some other small, reasonable time frame.
--
CREATE TABLE user_password_resets (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Password hash "primary key"
  -- (ie. for the future when we have a support dashboard)
  token VARCHAR(32) NOT NULL,

  -- Foreign key to user
  user_token VARCHAR(32) NOT NULL,

  -- Secret "key" for the password reset
  -- This will be shared via URL or user text input.
  public_reset_token VARCHAR(32) NOT NULL,

  -- Copied from the user record at the time of password reset issuance.
  -- If the user's password version is greater than this value, then this
  -- reset is no longer valid.
  current_password_version INT UNSIGNED NOT NULL DEFAULT 0,

  -- Whether the password reset has been consumed.
  is_redeemed BOOLEAN NOT NULL DEFAULT false,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  ip_address_creation VARCHAR(40) NOT NULL,
  ip_address_redemption VARCHAR(40) DEFAULT NULL,

  -- Incremented with every update.
  version INT UNSIGNED NOT NULL DEFAULT 0,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- When the password reset expires
  expires_at TIMESTAMP NOT NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  UNIQUE KEY (public_reset_token),

  KEY fk_user_token (user_token),
  KEY index_secret_key (secret_key),
  KEY index_expires_at (expires_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
