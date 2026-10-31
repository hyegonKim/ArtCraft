-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE voice_conversion_results (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- ========== INFERENCE DETAILS ==========

  -- The TTS model that was used
  model_token VARCHAR(32) NOT NULL,

  -- If set, specifies which vocoder was used.
  maybe_vocoder_token VARCHAR(64) DEFAULT NULL,

 -- Which media token was used (can be audio or video)
  media_token VARCHAR(32) NOT NULL,

  -- ========== CREATOR DETAILS ==========

  -- The person that created the result
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

  -- Full path where the wav, spectrogram, video, etc. are located.
  public_bucket_directory_full_path VARCHAR(255) NOT NULL,

  -- Name of the wav audio file (if it exists)
  public_bucket_wav_audio_object_name VARCHAR(255) NOT NULL,

  -- Name of the spectrogram (if it exists)
  public_bucket_spectrogram_object_name VARCHAR(255) NOT NULL,

  -- Name of the video (if it exists)
  public_bucket_video_object_name VARCHAR(255) NOT NULL,

  -- ========== METADATA ==========

  file_size_bytes INT(10) NOT NULL DEFAULT 0,
  duration_millis INT(10) NOT NULL DEFAULT 0,

  -- ========== INFERENCE METADATA, DC, ROUTING, DEBUGGING ==========

  -- If true, the request was routed to a special "debug" worker.
  is_debug_request BOOLEAN NOT NULL DEFAULT FALSE,

  is_generated_on_prem BOOLEAN NOT NULL DEFAULT FALSE,
  generated_by_worker VARCHAR(255) DEFAULT NULL,

  -- ========== MODERATION DETAILS ==========

  -- The last user (possibily moderator) that made changes.
  -- See the future audit logs table for better info and accounting.
  maybe_last_update_user_token VARCHAR(32) DEFAULT NULL,

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
  KEY fk_maybe_vocoder_token (maybe_vocoder_token),
  KEY fk_media_token (media_token),
  KEY fk_maybe_creator_user_token (maybe_creator_user_token),
  KEY fk_maybe_creator_anonymous_visitor_token (maybe_creator_anonymous_visitor_token),
  KEY index_creator_ip_address (creator_ip_address),
  KEY index_user_deleted_at(user_deleted_at),
  KEY index_mod_deleted_at(mod_deleted_at),
  KEY index_created_at(created_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
