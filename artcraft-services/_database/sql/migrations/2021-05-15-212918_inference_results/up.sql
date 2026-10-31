-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- NB: See the "squashed" copy of this for better documentation
-- and the current state of the table.

CREATE TABLE tts_results (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- ========== INFERENCE DETAILS ==========

  -- The TTS model that was used
  model_token VARCHAR(32) NOT NULL,

  -- NB: ADDED BY ALTER
  -- maybe_pretrained_vocoder_used VARCHAR(64) DEFAULT NULL,

  -- The original raw, unprocessed user input.
  raw_inference_text TEXT NOT NULL,

  -- SHA2 hash of the text [SHA2 = CHAR(64), SHA1 = CHAR(40), MD5 = CHAR(32)]
  raw_inference_text_hash_sha2 CHAR(64) NOT NULL,

  -- First pass at text normalization.
  -- eg. 14th -> fourteenth, etc.
  normalized_inference_text TEXT NOT NULL,

  -- If the model uses arpabet, we'll save the arpabet we used.
  maybe_arpabet_text TEXT DEFAULT NULL,

  -- ========== CREATOR DETAILS ==========

  -- The person that created the template.
  -- If the user wasn't logged in, this is null
  maybe_creator_user_token VARCHAR(32) DEFAULT NULL,

  -- Based on a cookie sent by the frontend.
  -- We'll save this even if the user is logged in.
  maybe_creator_anonymous_visitor_token VARCHAR(32) DEFAULT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  creator_ip_address VARCHAR(40) NOT NULL,

  -- (THIS MIGHT NOT BE USED)
  -- NB: DO NOT SORT!
  -- THIS MUST MATCH THE RESPECTIVE JOBS TABLE.
  creator_set_visibility ENUM(
    'public',
    'hidden',
    'private'
  ) NOT NULL DEFAULT 'public',

  -- The synthetic id associated with this result.
  -- These ids are incremented on a per-user basis to help users
  -- sequence their own work. They serve no other purpose.
  maybe_creator_synthetic_id BIGINT(20) DEFAULT NULL,

  -- ========== BUCKET STORAGE ==========

  -- Where the wav, spectrogram, and etc. are located.
  public_bucket_wav_audio_path VARCHAR(255) NOT NULL,
  public_bucket_spectrogram_path VARCHAR(255) NOT NULL,

  -- ========== METADATA ==========

  file_size_bytes INT(10) NOT NULL DEFAULT 0,
  duration_millis INT(10) NOT NULL DEFAULT 0,

  -- ========== MODERATION DETAILS ==========

  -- The last moderator that made changes.
  maybe_mod_user_token VARCHAR(32) DEFAULT NULL,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- If this is removed by a mod or the creator.
  user_deleted_at TIMESTAMP NULL,
  mod_deleted_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  KEY fk_model_token (model_token),
  KEY fk_maybe_creator_user_token (maybe_creator_user_token),
  KEY fk_maybe_creator_anonymous_visitor_token (maybe_creator_anonymous_visitor_token),
  KEY fk_maybe_mod_user_token (maybe_mod_user_token),
  KEY index_creator_ip_address (creator_ip_address),
  KEY index_user_deleted_at(user_deleted_at),
  KEY index_mod_deleted_at(mod_deleted_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

CREATE TABLE w2l_results (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- ========== INFERENCE DETAILS ==========

  -- The w2l template that was used (if set)
  maybe_w2l_template_token VARCHAR(32) NOT NULL,

  -- The inference result, if we're using them.
  maybe_tts_inference_result_token VARCHAR(32) DEFAULT NULL,

  -- ========== CREATOR DETAILS ==========

  -- The person that created the template.
  -- If the user wasn't logged in, this is null
  maybe_creator_user_token VARCHAR(32) DEFAULT NULL,

  -- Based on a cookie sent by the frontend.
  -- We'll save this even if the user is logged in.
  maybe_creator_anonymous_visitor_token VARCHAR(32) DEFAULT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  creator_ip_address VARCHAR(40) NOT NULL,

  -- (THIS MIGHT NOT BE USED)
  -- NB: DO NOT SORT!
  -- THIS MUST MATCH THE RESPECTIVE JOBS TABLE.
  creator_set_visibility ENUM(
    'public',
    'hidden',
    'private'
  ) NOT NULL DEFAULT 'public',

  -- The synthetic id associated with this result.
  -- These ids are incremented on a per-user basis to help users
  -- sequence their own work. They serve no other purpose.
  maybe_creator_synthetic_id BIGINT(20) DEFAULT NULL,

  -- ========== BUCKET STORAGE ==========

  -- Where the wav, spectrogram, and etc. are located.
  public_bucket_video_path VARCHAR(255) NOT NULL,

  -- ========== METADATA ==========

  file_size_bytes INT(10) NOT NULL DEFAULT 0,
  mime_type VARCHAR(32) NOT NULL DEFAULT '',
  frame_width INT(10) NOT NULL DEFAULT 0,
  frame_height INT(10) NOT NULL DEFAULT 0,
  duration_millis INT(10) NOT NULL DEFAULT 0,

  -- ========== MODERATION DETAILS ==========

  -- The last moderator that made changes.
  maybe_mod_user_token VARCHAR(32) DEFAULT NULL,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- If this is removed by a mod or the creator.
  user_deleted_at TIMESTAMP NULL,
  mod_deleted_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  KEY fk_maybe_creator_user_token (maybe_creator_user_token),
  KEY fk_maybe_creator_anonymous_visitor_token (maybe_creator_anonymous_visitor_token),
  KEY fk_maybe_w2l_template_token (maybe_w2l_template_token),
  KEY fk_maybe_tts_inference_result_token (maybe_tts_inference_result_token),
  KEY fk_maybe_mod_user_token (maybe_mod_user_token),
  KEY index_creator_ip_address (creator_ip_address),
  KEY index_user_deleted_at(user_deleted_at),
  KEY index_mod_deleted_at(mod_deleted_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
