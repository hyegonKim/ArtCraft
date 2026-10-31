-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- How to respond to different types of Twitch events.
-- Multiple rows per user
CREATE TABLE twitch_event_rules(
  -- Not used for anything except replication.
  id BIGINT(20) UNSIGNED NOT NULL AUTO_INCREMENT,

  -- Idempotency token from client
  -- This is so the frontend client doesn't submit duplicate jobs.
  uuid_idempotency_token VARCHAR(36) NOT NULL,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- Foreign key to user
  -- This is a FakeYou/Storyteller account, *NOT* a twitch user id.
  user_token VARCHAR(32) NOT NULL,

  -- ========== EVENT MATCHING AND BEHAVIOR ==========

  -- What type of event we'll be responding to.
  event_category ENUM(
      'bits',
      'channel_points',
      'chat_command'
  ) NOT NULL,

  -- A JSON payload containing any predicates we wish to apply to the match.
  -- eg. bits_value > 100
  -- eg. channel_points_award_name == "Mario Voice"
  event_match_predicate MEDIUMTEXT NOT NULL,

  -- A JSON payload containing how we wish to respond to the event.
  -- eg. tts M:model
  event_response MEDIUMTEXT NOT NULL,

  -- ========== RULE ORGANIZATION AND MANAGEMENT ==========

  -- The user can rearrange the rules in the UI.
  -- This will be the order they apply in if matched.
  -- NB: There is nothing in MySQL to guarantee unique ordering.
  user_specified_rule_order INT(10) UNSIGNED NOT NULL DEFAULT 0,

  -- Whether or not the rule is enabled.
  -- This is different than deleted. It still shows up in the UI.
  rule_is_disabled BOOLEAN NOT NULL DEFAULT FALSE,

  -- ========== ABUSE TRACKING, ETC. ==========

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
  UNIQUE KEY (token),
  KEY fk_user_token (user_token),
  KEY index_event_category (event_category),
  KEY index_deleted_at (deleted_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
