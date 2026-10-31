-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- One row per user. Contains all of the general (non-repeated) configs.
CREATE TABLE twitch_general_settings (
  -- Not used for anything except replication.
  id BIGINT(20) UNSIGNED NOT NULL AUTO_INCREMENT,

  -- Foreign key to user
  -- This is a FakeYou/Storyteller account, *NOT* a twitch user id.
  user_token VARCHAR(32) NOT NULL,

  -- Kill switch for the entire service.
  storyteller_is_enabled BOOLEAN NOT NULL DEFAULT FALSE,

  -- Kill switch for TTS.
  tts_is_enabled BOOLEAN NOT NULL DEFAULT FALSE,

  -- Kill switch for W2L.
  w2l_is_enabled BOOLEAN NOT NULL DEFAULT FALSE,

  -- Can set a fallback voice if a TTS voice goes missing.
  maybe_fallback_tts_model_token VARCHAR(32) DEFAULT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  ip_address_creation VARCHAR(40) NOT NULL,
  ip_address_last_update VARCHAR(40) NOT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (user_token)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
