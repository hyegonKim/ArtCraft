-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- This table contains both user uploads and inference or processing results.
CREATE TABLE media_files (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- We don't need an idempotency token!
  -- Store upload idempotency tokens in a separate table.
  -- uuid_idempotency_token VARCHAR(36) NOT NULL,

  -- ========== FOREIGN KEY TO ORIGIN ==========

  -- Where the file came from:
  --   * 'inference' for inference output
  --   * 'processed' for processed file
  --   * 'upload' for direct user upload
  origin_category VARCHAR(16) NOT NULL,

  -- For inference that can be tied back to a model, the type of model.
  --   * 'sad_talker', which doesn't have a value for maybe_origin_model_token (!!!)
  --   * (more tome come)
  maybe_origin_model_type VARCHAR(32) DEFAULT NULL,

  -- For inference that can be tied back to a model, the token of the model.
  -- For zero shot models, this may be set as the token for the vector.
  maybe_origin_model_token VARCHAR(32) DEFAULT NULL,

  -- The original filename of the media (if uploaded by a user)
  maybe_origin_filename VARCHAR(255) DEFAULT NULL,

  -- ========== METADATA ==========

  -- We can store text transcripts, etc. in other tables and refer back via foreign key.
  -- Titles and even descriptions can go in another table(s): media_file_annotations, media_file_tags, media_file_descriptions, ...
  -- (Not everything would have a text transcript.)

  -- ========== MEDIA DETAILS ==========

  -- Type of media:
  --   * 'audio' for wav, mp3, etc.
  --   * 'image' for a variety of video types.
  --   * 'video' for a variety of video types.
  media_type VARCHAR(16) NOT NULL,

  -- The file's mime type.
  maybe_mime_type VARCHAR(32) DEFAULT NULL,

  -- The file size.
  file_size_bytes INT(10) NOT NULL DEFAULT 0,

  -- The duration in millis, if audio or video.
  maybe_duration_millis INT(10) DEFAULT NULL,

  -- Audio encoder details
  -- Only present if the file was audio or a video that had audio.
  maybe_audio_encoding VARCHAR(32) DEFAULT NULL,

  -- Video encoder details
  -- Only present if the file was a video.
  maybe_video_encoding VARCHAR(32) DEFAULT NULL,

  -- For videos, the original frame width.
  maybe_frame_width INT(5) DEFAULT NULL,

  -- For videos, the original frame height.
  maybe_frame_height INT(5) DEFAULT NULL,

  -- Checksum of the original media
  -- SHA1 hash [SHA2 = CHAR(64), SHA1 = CHAR(40), MD5 = CHAR(32)]
  -- checksum_sha1 CHAR(40) NOT NULL,

  -- Checksum of the original media
  -- SHA2 hash [SHA2 = CHAR(64), SHA1 = CHAR(40), MD5 = CHAR(32)]
  -- Note that SHA2 is a hash family (SHA-228, SHA-256, SHA-384, SHA-512, SHA-512/224, SHA-512/256, ...)
  -- and produces different lengths output depending on the choice of algorithm.
  checksum_sha2 CHAR(64) NOT NULL,

  -- ========== UPLOAD, TRANSCODING, AND TRUNCATION DETAILS ==========

  -- The hash for the bucket directory that contains the original upload
  -- as well as any transcodings, downsamplings, etc.
  public_bucket_directory_hash  VARCHAR(32) NOT NULL,

  -- The directory this media is uploaded to will be exclusive for this file.
  -- Only this given record will live in this bucket, but the directory may include
  -- other transcodings or truncations.
  public_bucket_directory_full_path VARCHAR(255) NOT NULL,

  -- We'll likely transcode (and potentially truncate) most media given to us.
  -- This will store a json-encoded struct that details the changes.
  -- TEXT = 65,535 bytes (64 KiB), ~= 4 bytes per UTF-8 character, ~ 16383 characters.
  extra_file_modification_info TEXT DEFAULT NULL,

  -- ========== CREATOR DETAILS AND PREFERENCES ==========

  -- Foreign key to user
  maybe_creator_user_token VARCHAR(32) DEFAULT NULL,

  -- Based on a cookie sent by the frontend.
  -- We'll save this even if the user is logged in.
  maybe_creator_anonymous_visitor_token VARCHAR(32) DEFAULT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  creator_ip_address VARCHAR(40) NOT NULL,

  -- The creator can set a desired visibility for their data.
  -- This does not always apply to every upload type.
  -- Additionally, some upload types may require moderator approval prior
  -- to being publicly listed, and this field has no bearing on that.
  -- NB: DO NOT CHANGE THE ORDER OF THE ENUM VALUES DURING SCHEMA MIGRATIONS.
  creator_set_visibility ENUM(
    'public',
    'hidden',
    'private'
  ) NOT NULL DEFAULT 'public',

  -- The synthetic id associated with this result.
  -- These ids are incremented on a per-user basis to help users
  -- sequence their own work. They serve no other purpose.
  maybe_creator_synthetic_id BIGINT(20) DEFAULT NULL,

  -- ========== MODERATION DETAILS ==========

  -- The last moderator that made changes last.
  -- We should also build an audit table to supersede this.
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
  KEY index_origin_category (origin_category),
  KEY index_maybe_origin_model_type (maybe_origin_model_type),
  KEY fk_maybe_origin_model_token (maybe_origin_model_token),
  KEY fk_maybe_origin_model_type_and_token (maybe_origin_model_type, maybe_origin_model_token),
  KEY index_media_type (media_type),
  KEY index_checksum_sha2 (checksum_sha2),
  KEY fk_maybe_creator_user_token (maybe_creator_user_token),
  KEY fk_maybe_mod_user_token (maybe_mod_user_token),
  KEY index_creator_ip_address (creator_ip_address),
  KEY index_user_deleted_at (user_deleted_at),
  KEY index_mod_deleted_at (mod_deleted_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
