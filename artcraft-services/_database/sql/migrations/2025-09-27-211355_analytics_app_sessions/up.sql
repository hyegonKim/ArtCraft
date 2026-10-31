-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Track user session durations
-- User clients ping the endpoint that writes this data.
CREATE TABLE analytics_app_sessions (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Generated client-side
  -- Wide enough for a UUID, but typically one of our internal tokens.
  session_token VARCHAR(36) NOT NULL,

  -- If we host multiple sites with distinct categories, this will enable us to
  -- segregate analytics.
  app_namespace VARCHAR(32) NOT NULL,

  -- Who sent the event.
  user_token VARCHAR(32) NOT NULL,

  -- Updated on every write.
  -- The "user agent" or app version string of the client.
  app_version VARCHAR(255) DEFAULT NULL,

  -- Updated on every write.
  -- The OS platform for the client.
  os_platform VARCHAR(16) DEFAULT NULL,

  -- Updated on every write.
  -- The OS platform version for the client.
  os_version VARCHAR(64) DEFAULT NULL,

  -- Updated on every write.
  -- How long the current user's session has been open
  session_duration_seconds INT(10) UNSIGNED DEFAULT NULL,

  -- Updated on every write.
  -- The user's last known IP address.
  ip_address VARCHAR(40) NOT NULL,

  -- Incrementing count of events.
  -- NB: This is kind of an "hours spent" measure, but will lose information if we ever
  -- change the ping cadence (eg. 1 minute pings --> 5 minute pings).
  measurement_count BIGINT UNSIGNED NOT NULL DEFAULT 0,

  -- ========== RECORD TIMESTAMPS ==========

  -- Written at create only.
  first_active_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

  -- Updated on every ping.
  last_active_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

  -- ========== INDICES ==========
  PRIMARY KEY (id),
  UNIQUE KEY (session_token),
  INDEX idx_app_namespace (app_namespace),
  INDEX idx_user_token (user_token),
  INDEX idx_os_platform (os_platform),
  INDEX idx_os_version (os_version),
  INDEX idx_app_version (app_version),
  INDEX idx_last_active_at (last_active_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
