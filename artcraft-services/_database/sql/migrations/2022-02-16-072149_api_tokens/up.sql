-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- We'll use application logic to kill any API keys beyond the most recent 5 "active" a user creates.
CREATE TABLE api_tokens (
  -- Not used for anything except replication.
  id BIGINT(20) UNSIGNED NOT NULL AUTO_INCREMENT,

  -- Idempotency token from client
  -- This is so the frontend client doesn't submit duplicate jobs.
  uuid_idempotency_token VARCHAR(36) NOT NULL,

  -- This is a private internal reference to the API token.
  -- Not shared with consumers / users.
  -- This should be used as the foreign key for the token.
  internal_token VARCHAR(32) NOT NULL,

  -- This is the secret API token value given to consumers
  api_token VARCHAR(32) NOT NULL,

  -- Foreign key to user that owns the API token.
  -- A user can have several.
  user_token VARCHAR(32) NOT NULL,

  -- A user-defined short description (optional)
  maybe_short_description VARCHAR(32) DEFAULT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  ip_address_creation VARCHAR(40) NOT NULL,
  ip_address_last_update VARCHAR(40) NOT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  deleted_at TIMESTAMP DEFAULT NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (uuid_idempotency_token),
  UNIQUE KEY (internal_token),
  UNIQUE KEY (api_token),
  KEY fk_user_token (user_token),
  KEY index_created_at (created_at),
  KEY index_deleted_at (deleted_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
