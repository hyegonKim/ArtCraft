-- NB: This is a manually squashed view of all the CREATE and ALTER statements,
-- with comments attached to the fields for centralized documentation.

-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- These are the individual files in each dataset.
-- Files can be added and removed from a dataset at any time.
CREATE TABLE zs_voice_dataset_samples (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" for joins, API, etc. (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- Idempotency token from client
  -- This is so the frontend client doesn't submit duplicate uploads.
  uuid_idempotency_token VARCHAR(36) NOT NULL,

  -- ========== ZERO SHOT FOREIGN KEYS AND STATE==========

  -- The zero shot dataset that was used to create this voice.
  -- The dataset will only be accessible to the original author of the voice.
  -- The original author can create multiple voices from the same dataset.
  dataset_token VARCHAR(32) NOT NULL,

  -- Foreign key to the uploaded media file.
  -- There was an option of handling uploads and file management separately, but by electing to store the dataset
  -- samples in the "media_files" table, they can be reused by our other code, queries, machinery, etc.
  -- Note that this requires that the "media_files" record must be an audio (not video) file (which our code will
  -- enforce), and that it is given a category of "voice_zero_shot" so it isn't co-mingled with other uploads/outputs
  media_file_token VARCHAR(32) NOT NULL,

  -- Some data sets require raw transcripts for the audio.
  -- If required, they'll live here.
  maybe_raw_transcription TEXT DEFAULT NULL,

  -- A way to order the data set samples
  -- We don't necessarily need to reimplement file reordering.
  -- order_id INT DEFAULT NULL,

  -- ========== CREATOR DETAILS AND PREFERENCES ==========

  -- Foreign key to user
  maybe_creator_user_token VARCHAR(32) DEFAULT NULL,

  -- Based on a cookie sent by the frontend.
  -- We'll save this even if the user is logged in.
  maybe_creator_anonymous_visitor_token VARCHAR(32) DEFAULT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  creator_ip_address VARCHAR(40) NOT NULL,

  -- ========== MODERATION DETAILS ==========

  -- The last moderator that made changes last.
  -- Fine details of moderator actions are kept in a separate `audit_logs` table.
  maybe_mod_user_token VARCHAR(32) DEFAULT NULL,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- If this is removed by a mod or the creator.
  user_deleted_at TIMESTAMP NULL,
  mod_deleted_at TIMESTAMP NULL,

  -- ========== VECTOR CLOCK ==========

  -- Incremented with every update.
  version INT NOT NULL DEFAULT 0,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  UNIQUE KEY (uuid_idempotency_token),

  KEY fk_dataset_token (dataset_token),
  KEY fk_media_file_token (media_file_token),
  KEY fk_maybe_creator_user_token (maybe_creator_user_token),
  KEY fk_maybe_mod_user_token (maybe_mod_user_token),
  KEY index_creator_ip_address (creator_ip_address),
  KEY index_user_deleted_at (user_deleted_at),
  KEY index_mod_deleted_at (mod_deleted_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
