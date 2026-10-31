-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- This is primarily meant to be for analysis.
-- NB: See [_docs/database_schema.md] for details on Twitch-specifics.

CREATE TABLE twitch_channel_point_events (
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

  -- ========== CHANNEL POINTS DETAILS ==========

  -- This is the title used for the reward
  -- We'll match on this to do TTS
  title VARCHAR(256) NOT NULL,

  -- This is the description used for the reward
  prompt VARCHAR(256) NOT NULL,

  -- This is the text that the user sends
  -- This is optional, but we'll coerce null to empty
  user_text_input TEXT NOT NULL,

  -- A UUID for this instance of reward redemption
  redemption_id VARCHAR(36) NOT NULL,

  -- A UUID for the reward
  reward_id VARCHAR(36) NOT NULL,

  reward_cost INT(10) UNSIGNED DEFAULT NULL,

  is_sub_only BOOLEAN NOT NULL DEFAULT FALSE,
  max_per_stream INT(10) UNSIGNED DEFAULT NULL,
  max_per_user_per_stream INT(10) UNSIGNED DEFAULT NULL,

  -- INDICES --
  PRIMARY KEY (id),
  KEY index_sender_twitch_user_id (sender_twitch_user_id),
  KEY index_sender_twitch_username_lowercase (sender_twitch_username_lowercase),
  KEY index_destination_channel_id (destination_channel_id)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
