-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE voice_clone_requests(
  -- Not used for anything except replication.
  id BIGINT(20) UNSIGNED NOT NULL AUTO_INCREMENT,

  -- Idempotency token from client
  -- This is so the frontend client doesn't submit duplicate jobs.
  uuid_idempotency_token VARCHAR(36) NOT NULL,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- ========== CONTACT ==========

  -- Foreign key to user
  -- This is a FakeYou/Storyteller account, *NOT* a twitch user id.
  maybe_user_token VARCHAR(32) DEFAULT NULL,

  email_address VARCHAR(255) NOT NULL,

  discord_username VARCHAR(255) NOT NULL,

  -- ========== REASON ==========

  -- Visibility
  is_for_private_use BOOLEAN NOT NULL DEFAULT FALSE,
  is_for_public_use BOOLEAN NOT NULL DEFAULT FALSE,

  -- How they'll use
  is_for_studio BOOLEAN NOT NULL DEFAULT FALSE,
  is_for_twitch_tts BOOLEAN NOT NULL DEFAULT FALSE,
  is_for_api_use BOOLEAN NOT NULL DEFAULT FALSE,
  is_for_music BOOLEAN NOT NULL DEFAULT FALSE,
  is_for_games BOOLEAN NOT NULL DEFAULT FALSE,
  is_for_other BOOLEAN NOT NULL DEFAULT FALSE,
  optional_notes_on_use MEDIUMTEXT DEFAULT NULL,

  -- Subject/Ownership
  is_own_voice BOOLEAN NOT NULL DEFAULT FALSE,
  is_third_party_voice BOOLEAN NOT NULL DEFAULT FALSE,
  optional_notes_on_subject VARCHAR(255) DEFAULT NULL,

  -- ========== RECORDING QUALITY / EQUIPMENT ==========

  has_clean_audio_recordings BOOLEAN NOT NULL DEFAULT FALSE,
  has_good_microphone BOOLEAN NOT NULL DEFAULT FALSE,

  -- ========== COMMENTS ==========

  optional_questions MEDIUMTEXT DEFAULT NULL,
  optional_extra_comments MEDIUMTEXT DEFAULT NULL,

  -- ========== BUSINESS END / INTERNAL USE ==========

  internal_has_been_handled BOOLEAN NOT NULL DEFAULT FALSE,

  internal_notes MEDIUMTEXT DEFAULT NULL,

  -- ========== ABUSE TRACKING, ETC. ==========

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  ip_address_creation VARCHAR(40) NOT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (uuid_idempotency_token),
  UNIQUE KEY (token),
  KEY fk_maybe_user_token (maybe_user_token)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
