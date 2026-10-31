-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE moderator_audit_log_entries (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Visible "primary key"
  token VARCHAR(32) NOT NULL,

  -- The type of the event (ban, delete, etc.)
  event_type VARCHAR(32) NOT NULL,

  -- The target user
  maybe_target_user_token VARCHAR(32) DEFAULT NULL,

  -- The target "entity", which varies by event type.
  -- eg. a model token or template token
  maybe_target_entity_token VARCHAR(32) DEFAULT NULL,

  -- If an IP was banned, we record it here.
  maybe_target_ip_address VARCHAR(40) DEFAULT NULL,

  -- Optional mod-specified notes.
  optional_notes TEXT DEFAULT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  KEY fk_maybe_target_user_token (maybe_target_user_token),
  KEY fk_maybe_target_entity_token (maybe_target_entity_token),
  KEY index_event_type (event_type)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
