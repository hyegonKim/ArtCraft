-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE ip_address_bans (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- The IP address to ban
  ip_address VARCHAR(40) NOT NULL,

  -- Maybe the target user.
  maybe_target_user_token VARCHAR(32) DEFAULT NULL,

  -- Mod who created or last edited the ban.
  mod_user_token VARCHAR(32) NOT NULL,

  -- Optional notes about the ban.
  mod_notes TEXT NOT NULL,

  -- Moderator can set an expire time (optional).
  -- If not set, the ban is indefinite
  expires_at TIMESTAMP DEFAULT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- If deleted, the ban is removed.
  deleted_at TIMESTAMP DEFAULT NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (ip_address),
  KEY fk_mod_user_token (mod_user_token),
  KEY fk_maybe_target_user_token (maybe_target_user_token)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
