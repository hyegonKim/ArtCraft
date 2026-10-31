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

 -- Which media token was used as input (can be audio or video), potentially
 -- from a wide number of tables (eg. media_uploads, tts_results, etc.)
  media_token VARCHAR(32) NOT NULL,

  -- For now we just support the `media_uploads` table, but we may grow to
  -- support media inputs from other areas of the system (eg. tts, video, etc.)
  media_token_type VARCHAR(32) NOT NULL,

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

  -- The hash lets us reconstitute the bucket directory
  -- location (which stores potentially several files)
  public_bucket_hash VARCHAR(64) NOT NULL,

  -- Whether we're storing a wav result file.
  bucket_has_wav BOOLEAN NOT NULL DEFAULT FALSE,

  -- Whether we're storing a mp3 result file.
  bucket_has_mp3 BOOLEAN NOT NULL DEFAULT FALSE,

  -- Whether we're storing a mp4 result file.
  bucket_has_mp4 BOOLEAN NOT NULL DEFAULT FALSE,

  -- Whether we're storing a webm result file.
  bucket_has_webm BOOLEAN NOT NULL DEFAULT FALSE,

  -- Whether we're storing a spectrogram json file.
  bucket_has_spectrogram BOOLEAN NOT NULL DEFAULT FALSE,

  -- ========== METADATA ==========

  file_size_bytes INT(10) NOT NULL DEFAULT 0,
  duration_millis INT(10) NOT NULL DEFAULT 0,

  -- ========== INFERENCE METADATA, DC, ROUTING, DEBUGGING ==========

  -- If true, the request was routed to a special "debug" worker.
  is_debug_request BOOLEAN NOT NULL DEFAULT FALSE,

  -- Whether we generated this result from our own data center (vs. cloud).
  is_generated_on_prem BOOLEAN NOT NULL DEFAULT FALSE,

  -- Worker hostname (linux hostname, k8s pod name)
  generated_by_worker VARCHAR(255) DEFAULT NULL,

  -- Cluster name (k8s)
  generated_by_cluster VARCHAR(255) DEFAULT NULL,

  -- ========== MODERATION DETAILS ==========

  -- The last user (possibly moderator) that made changes.
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
  KEY fk_media_token (media_token),
  KEY fk_media_token_and_media_token_type (media_token, media_token_type),
  KEY fk_maybe_creator_user_token (maybe_creator_user_token),
  KEY fk_maybe_creator_anonymous_visitor_token (maybe_creator_anonymous_visitor_token),
  KEY index_creator_ip_address (creator_ip_address),
  KEY index_user_deleted_at(user_deleted_at),
  KEY index_mod_deleted_at(mod_deleted_at),
  KEY index_created_at(created_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
