-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE browser_session_logs (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- ========== PRIMARY KEY ==========

  -- We'll generate this on the first call
  token VARCHAR(32) NOT NULL,

  -- ========== USER ==========

  -- IP address is only written on the first request.
  ip_address VARCHAR(40) NOT NULL,

  maybe_user_token VARCHAR(32) DEFAULT NULL,

  maybe_anonymous_visitor_token VARCHAR(32) DEFAULT NULL,

  -- ========== DETAILS ==========

  -- We might intentionally record some actions by name
  maybe_last_action VARCHAR(32) DEFAULT NULL,

  -- ========== VECTOR CLOCK ==========

  -- Incremented with every update.
  update_count INT UNSIGNED NOT NULL DEFAULT 0,

  -- ========== TIMESTAMPS ==========

  -- When the key was created
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

  -- When the log was last updated
  maybe_last_updated_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),

  KEY index_maybe_user_token (maybe_user_token),
  KEY index_update_count (update_count),
  KEY index_created_at (created_at),
  KEY index_maybe_last_updated_at (maybe_last_updated_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
