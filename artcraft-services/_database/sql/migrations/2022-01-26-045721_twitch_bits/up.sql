-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- This is primarily meant to be for analysis.
-- NB: See [_docs/database_schema.md] for details on Twitch-specifics.

CREATE TABLE twitch_bits_events(
  -- Not used for anything except replication.
  id BIGINT(20) UNSIGNED NOT NULL AUTO_INCREMENT,

  -- ========== USER / CHANNEL ==========

  -- The user sending the bits.
  -- We'll need to keep the name and lowercase for lookup.
  sender_twitch_user_id VARCHAR(64) NOT NULL,
  sender_twitch_username VARCHAR(32) NOT NULL,
  sender_twitch_username_lowercase VARCHAR(32) NOT NULL,

  -- The channel the bits were being sent to.
  -- We technically only need the ID since we can JOIN, but we'll keep
  -- the name for convenience.
  destination_channel_id VARCHAR(64) NOT NULL,
  destination_channel_name VARCHAR(32) NOT NULL,

  -- ========== BITS DETAILS ==========

  is_anonymous BOOLEAN NOT NULL DEFAULT FALSE,

  -- The number of bits just spent
  bits_used INT(10) UNSIGNED DEFAULT NULL,

  -- The lifetime(?) number of bits spent by the user in this channel.
  total_bits_used INT(10) UNSIGNED DEFAULT NULL,

  chat_message TEXT NOT NULL,

  -- INDICES --
  PRIMARY KEY (id),
  KEY index_sender_twitch_user_id (sender_twitch_user_id),
  KEY index_sender_twitch_username_lowercase (sender_twitch_username_lowercase),
  KEY index_destination_channel_id (destination_channel_id)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
