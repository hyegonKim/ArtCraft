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

  -- A link to a possible thumbnail (TODO: Build thumbnail support reusing media_files)
  maybe_thumbnail_token VARCHAR(32) DEFAULT NULL,

  -- The description of the model in markdown.
  description_markdown TEXT NOT NULL,

  -- Generated HTML (not user-editable).
  description_rendered_html TEXT NOT NULL,


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

  -- File characteristics
  file_size_bytes INT(10) NOT NULL DEFAULT 0,

  -- Checksum of the original weights file
  -- SHA2 hash [SHA2 = CHAR(64), SHA1 = CHAR(40), MD5 = CHAR(32)]
  -- Note that SHA2 is a hash family (SHA-228, SHA-256, SHA-384, SHA-512, SHA-512/224, SHA-512/256, ...)
  -- and produces different lengths output depending on the choice of algorithm.
  file_checksum_sha2 CHAR(64) NOT NULL,

  -- ========== BUCKET STORAGE ==========

  -- The hash for the bucket directory that contains the original upload
  -- as well as any associated other files.
  -- The bucket filename for the primary file (not including the directory path) is given by:
  -- `[{maybe_private_bucket_prefix}]{private_bucket_hash}[{maybe_private_bucket_extension}]`
  private_bucket_hash  VARCHAR(32) NOT NULL,

  -- An optional prefix on the bucket filename.
  -- If present, this will be prepended to the beginning of the bucket filename to access the file.
  -- The bucket filename for the primary file (not including the directory path) is given by:
  -- `[{maybe_private_bucket_prefix}]{private_directory_hash}[{maybe_private_bucket_extension}]`
  maybe_private_bucket_prefix VARCHAR(16) DEFAULT NULL,

  -- An optional appended extension on the bucket filename.
  -- If present, this will be appended to the end of the bucket filename to access the file.
  -- To allow for flexibility, this extension typically will contain a leading period if
  -- the file needs it (eg ".mp4" rather than "mp4")!
  -- The bucket filename for the primary file (not including the directory path) is given by:
  -- `[{maybe_private_bucket_prefix}]{private_directory_hash}[{maybe_private_bucket_extension}]`
  maybe_private_bucket_extension VARCHAR(16) DEFAULT NULL,


  -- ========== MODERATION DETAILS ==========

  -- Mods may have to approve of vc models for them to show up in a public index.
  -- Models can be used by the author (or other parties that know the URL) before
  -- they're approved, but unapproved models won't show up in public indices.
  -- This is independent of other visibility controls (is_mod_disabled, deleted_at,
  -- and creator_set_visibility)
  -- is_public_listing_approved BOOLEAN DEFAULT NULL,

  -- Extremely popular models may be locked from deletion or modification by users.
  -- is_locked_from_user_modification BOOLEAN NOT NULL DEFAULT FALSE,


  -- ========== RATING AND RANKING ==========

  -- User ratings are cached on the model, but are *not* a source of truth.
  -- Total count only includes "positive" and "negative" votes, not neutral ones.
  cached_user_ratings_total_count INT(10) UNSIGNED NOT NULL DEFAULT 0,

  -- Total positive
  cached_user_ratings_positive_count INT(10) UNSIGNED NOT NULL DEFAULT 0,

  -- Total negative
  cached_user_ratings_negative_count INT(10) UNSIGNED NOT NULL DEFAULT 0,

  -- User ratings are cached on the model, but are *not* a source of truth.
  -- Total count only includes "positive" and "negative" votes, not neutral ones.
  maybe_cached_user_ratings_ratio FLOAT,

  -- When the cached ratings were last calculated and updated.
  cached_user_ratings_last_updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,


  -- ========== MODEL PLACEMENT ==========

  -- Whether the model features on the front of FakeYou
  -- is_front_page_featured BOOLEAN NOT NULL DEFAULT FALSE,


-- NB: I think this should actually move into polymorphic join tables, so that we don't create a telescopic list of
-- columns.
--
--  -- ========== TTS MODEL METADATA ==========
--
--  -- This is for Tacotron2, specifically.
--  -- The name of the type of pipeline to use, eg "legacy_fakeyou", "legacy_vocodes", "english_v1", etc.
--  -- New pipelines may be added at any point in the future and the source of truth for them will mostly
--  -- live in the `storyteller-ml` repository.
--  -- Older models and brand new models will have this field set to null, and we'll infer which value to
--  -- lazily backfill based on the creation date and language setting.
--  -- tts_text_pipeline_type VARCHAR(64) DEFAULT NULL,
--
--  -- If true, multiply the mel outputs before being vocoded by a globally default constant.
--  tts_use_default_mel_multiply_factor BOOLEAN NOT NULL DEFAULT FALSE,
--
--  -- If not null, multiply the mel outputs before being vocoded by this value.
--  -- This is used instead of the default if `tts_use_default_mel_multiply_factor` is
--  -- set (ie. `tts_use_default_mel_multiply_factor` is ignored and this custom value is used instead)
--  maybe_tts_custom_mel_multiply_factor DOUBLE DEFAULT NULL,
--
--  -- If set, use a different pretrained vocoder.
--  -- If not set, use the website default. (Currently HifiGan-SS)
--  maybe_tts_default_pretrained_vocoder VARCHAR(64) DEFAULT NULL,
--
--  -- If set, we'll use a custom vocoder (from table vocoder_models) instead of a default vocoder.
--  -- If not set, we'll use `maybe_default_pretrained_vocoder` (or the website default).
--  maybe_tts_custom_vocoder_token VARCHAR(32) DEFAULT NULL,
--
--
--  -- ========== VOICE CONVERSION MODEL METADATA ==========
--
--  -- RVC (v2) specific - whether the model has an index file associated with it
--  -- These files improve the quality of the results.
--  vc_has_index_file BOOLEAN NOT NULL DEFAULT FALSE,
--
--
--  -- ========== VOCODER MODEL METADATA ==========
--
--  -- Whether the vocoder is recommended for use
--  -- vocoder_is_staff_recommended BOOLEAN NOT NULL DEFAULT FALSE,
--
--
--  -- ========== VOICE MODEL METADATA (TTS and Voice Conversion Models) ==========
--
--  -- The full IETF BCP47 language tag (eg. en, en-US, es-419, ja-JP, pt, etc.)
--  -- (Not that it matters apart from categorization, since voice conversion is universal.)
--  voice_ietf_language_tag VARCHAR(64) NOT NULL DEFAULT 'en',
--
--  -- The IETF BCP47 language tag's primary language subtag (eg. "es-419" becomes "es")
--  -- (Not that it matters apart from categorization, since voice conversion is universal.)
--  voice_ietf_primary_language_subtag VARCHAR(12) NOT NULL DEFAULT 'en',
--
--  -- RVC (v2) specific - whether the model has an index file associated with it
--  -- These files improve the quality of the results.
--  voice_has_index_file BOOLEAN NOT NULL DEFAULT FALSE,


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
  UNIQUE KEY (token)
  -- TODO:
  -- KEY index_model_type (model_type),
  -- KEY fk_maybe_vocoder_token (maybe_vocoder_token),
  -- KEY fk_creator_user_token (creator_user_token),
  -- KEY index_creator_set_visibility (creator_set_visibility),
  -- KEY index_private_bucket_hash (private_bucket_hash),
  -- KEY index_is_public_listing_approved (is_public_listing_approved),
  -- KEY index_is_locked_from_user_modification (is_locked_from_user_modification)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
