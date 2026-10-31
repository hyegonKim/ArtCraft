-- NB: This is a manually squashed view of all the CREATE and ALTER statements,
-- with comments attached to the fields for centralized documentation.

-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Every type of fine tuned model that users can upload.
CREATE TABLE model_weights (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- ========== WEIGHTS CLASSIFICATION ==========

  -- The exact type of weights
  -- Currently supported values:
  --   * 'hifigan_tt2' - hifigan for Tacotron2; TTS vocoder
  --   * 'rvc_v2' - RVC (v2); voice conversion
  --   * 'sd_1.5' - Stable Diffusion 1.5; image generation
  --   * 'sdxl' - Stable Diffusion XL; image generation
  --   * 'so_vits_svc' - SVC; voice conversion
  --   * 'tt2' - Tacotron2; TTS
  weights_type VARCHAR(32) NOT NULL,

  -- The broader category of the weights
  -- Currently supported values:
  --   * 'image_generation' (SD1.5, SDXL)
  --   * 'text_to_speech' (TT2)
  --   * 'vocoder' (Hifigan)
  --   * 'voice_conversion' (RVCv2, SVC)
  weights_category VARCHAR(32) NOT NULL,


  -- ========== MODEL DESCRIPTION ==========

  -- The "name" of the model, which might be complicated.
  title VARCHAR(255) NOT NULL,

  -- The "cover" image is a media file of type image that severs as a small
  -- example of the model. Like album art or a user profile picture / avatar.
  maybe_cover_image_media_file_token VARCHAR(32) DEFAULT NULL,

  -- The description of the model in markdown.
  -- This is optional, since not all weights are of the same caliber (eg. zero shot models
  -- don't deserve a description).
  maybe_description_markdown TEXT DEFAULT NULL,

  -- Generated HTML (not user-editable).
  -- This is optional, since not all weights are of the same caliber (eg. zero shot
  -- models don't deserve a description).
  maybe_description_rendered_html TEXT DEFAULT NULL,

  -- OPTIONAL. Only for TTS and Voice Conversion models.
  -- The full IETF BCP47 language tag (eg. en, en-US, es-419, ja-JP, pt, etc.)
  -- (Not that it matters apart from categorization, since voice conversion is universal.)
  -- NB: There's another set of tables (model_weights_extension_voice_conversion_details, etc.) that
  -- may have older post-migration data for some models. We'll want to backfill this field from there
  -- and then make this one authoritative.
  maybe_ietf_language_tag VARCHAR(64) DEFAULT NULL,

  -- OPTIONAL. Only for TTS and Voice Conversion models.
  -- The IETF BCP47 language tag's primary language subtag (eg. "es-419" becomes "es")
  -- (Not that it matters apart from categorization, since voice conversion is universal.)
  -- NB: There's another set of tables (model_weights_extension_voice_conversion_details, etc.) that
  -- may have older post-migration data for some models. We'll want to backfill this field from there
  -- and then make this one authoritative.
  maybe_ietf_primary_language_subtag VARCHAR(12) DEFAULT NULL,


  -- ========== CREATOR DETAILS ==========

  -- The person that created the model.
  creator_user_token VARCHAR(32) NOT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6.
  -- IP address of creator at time of creation.
  creator_ip_address VARCHAR(40) NOT NULL,

  -- (THIS MIGHT NOT BE USED)
  -- NB: DO NOT SORT!
  -- THIS MUST MATCH THE RESPECTIVE JOBS TABLE.
  creator_set_visibility ENUM(
    'public',
    'hidden',
    'private'
  ) NOT NULL DEFAULT 'public',

  -- The synthetic id associated with this model.
  -- These ids are incremented on a per-user basis to help users
  -- sequence their own work. They serve no other purpose.
  -- "weights" synthetic ids are incremented over all media files
  -- creator_weights_synthetic_id BIGINT(20) DEFAULT NULL,

  -- "weights_type" synthetic ids are incremented over a given weight type
  -- creator_weights_type_synthetic_id BIGINT(20) DEFAULT NULL,

  -- The last user to edit the model. This could be the creator or a moderator.
  -- Check the future audit logs table for more info.
  maybe_last_update_user_token VARCHAR(32) DEFAULT NULL,


  -- ========== IMMUTABLE PROVENANCE AND METADATA ==========

  -- Where the file was originally downloaded (if it was downloaded)
  original_download_url VARCHAR(512) DEFAULT NULL,

  -- The filename that was used at upload time (if available)
  original_filename VARCHAR(255) DEFAULT NULL,

  -- File size. BIGINT to store more than 2GB size.
  file_size_bytes BIGINT(10) NOT NULL DEFAULT 0,

  -- Checksum of the original weights file
  -- SHA2 hash [SHA2 = CHAR(64), SHA1 = CHAR(40), MD5 = CHAR(32)]
  -- Note that SHA2 is a hash family (SHA-228, SHA-256, SHA-384, SHA-512, SHA-512/224, SHA-512/256, ...)
  -- and produces different lengths output depending on the choice of algorithm.
  file_checksum_sha2 CHAR(64) NOT NULL,

  -- ========== BUCKET STORAGE ==========

  -- The hash for the bucket directory that contains the original upload
  -- as well as any associated other files.
  -- The bucket filename for the primary file (not including the directory path) is given by:
  -- `[{maybe_public_bucket_prefix}]{public_bucket_hash}[{maybe_public_bucket_extension}]`
  public_bucket_hash  VARCHAR(32) NOT NULL,

  -- An optional prefix on the bucket filename.
  -- If present, this will be prepended to the beginning of the bucket filename to access the file.
  -- The bucket filename for the primary file (not including the directory path) is given by:
  -- `[{maybe_public_bucket_prefix}]{public_bucket_hash}[{maybe_public_bucket_extension}]`
  maybe_public_bucket_prefix VARCHAR(16) DEFAULT NULL,

  -- An optional appended extension on the bucket filename.
  -- If present, this will be appended to the end of the bucket filename to access the file.
  -- To allow for flexibility, this extension typically will contain a leading period if
  -- the file needs it (eg ".mp4" rather than "mp4")!
  -- The bucket filename for the primary file (not including the directory path) is given by:
  -- `[{maybe_public_bucket_prefix}]{public_bucket_hash}[{maybe_public_bucket_extension}]`
  maybe_public_bucket_extension VARCHAR(16) DEFAULT NULL,

  -- This is a migration flag that denotes whether this model weight is stored in AWS.
  -- If not, it's stored in GCP.
  is_in_aws BOOLEAN NOT NULL DEFAULT false,

  -- This is a migration flag that denotes that weight is not stored in GCP.
  is_not_in_gcp BOOLEAN NOT NULL DEFAULT false,

  -- ========== MODERATION DETAILS ==========

  -- Mods may have to approve of vc models for them to show up in a public index.
  -- Models can be used by the author (or other parties that know the URL) before
  -- they're approved, but unapproved models won't show up in public indices.
  -- This is independent of other visibility controls (is_mod_disabled, deleted_at,
  -- and creator_set_visibility)
  -- is_public_listing_approved BOOLEAN DEFAULT NULL,

  -- Extremely popular models may be locked from deletion or modification by users.
  -- is_locked_from_user_modification BOOLEAN NOT NULL DEFAULT FALSE,

  -- ========== MIGRATION DETAILS ==========

  -- A cache of the total number of times the model has been used.
  -- We calculate this from `media_files` and legacy `tts_results` counts in another
  -- process and periodically update this figure. This is not a source of truth, nor
  -- should it be considered accurate, up to date, or monotonically increasing.
  cached_usage_count BIGINT UNSIGNED NOT NULL DEFAULT 0,

  -- ========== MIGRATION DETAILS ==========

  -- If migrated from another table, this is the token of the original model.
  maybe_migration_old_model_token VARCHAR(32) DEFAULT NULL,


  -- ========== VECTOR CLOCK ==========

  -- Incremented with every update.
  version INT NOT NULL DEFAULT 0,


  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- If this is removed by a mod.
  -- It completely disappears from the system.
  user_deleted_at TIMESTAMP NULL,
  mod_deleted_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  UNIQUE KEY unique_index_maybe_migration_old_model_token (maybe_migration_old_model_token),

  KEY index_weights_type (weights_type),
  KEY index_weights_category (weights_category),
  KEY fk_creator_user_token (creator_user_token),
  KEY index_creator_set_visibility (creator_set_visibility),
  KEY fk_maybe_cover_media_file_token (maybe_cover_image_media_file_token),
  KEY index_is_in_aws (is_in_aws),
  KEY index_updated_at (updated_at),
  KEY index_user_deleted_at (user_deleted_at),
  KEY index_mod_deleted_at (user_deleted_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
