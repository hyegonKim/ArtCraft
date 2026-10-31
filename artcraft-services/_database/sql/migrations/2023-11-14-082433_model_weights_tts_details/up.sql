-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Extra metadata on tts models
CREATE TABLE model_weights_tts_details (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Simultaneous primary and foreign key.
  -- This should match a single record in the `model_weights` table.
  -- This turns this table into a polymorphic extension table.
  model_weights_token VARCHAR(32) NOT NULL,

  -- ========== TTS MODEL METADATA ==========

  -- The full IETF BCP47 language tag (eg. en, en-US, es-419, ja-JP, pt, etc.)
  -- (Not that it matters apart from categorization, since voice conversion is universal.)
  ietf_language_tag VARCHAR(64) NOT NULL DEFAULT 'en',

  -- The IETF BCP47 language tag's primary language subtag (eg. "es-419" becomes "es")
  -- (Not that it matters apart from categorization, since voice conversion is universal.)
  ietf_primary_language_subtag VARCHAR(12) NOT NULL DEFAULT 'en',

  -- This is for Tacotron2, specifically.
  -- The name of the type of pipeline to use, eg "legacy_fakeyou", "legacy_vocodes", "english_v1", etc.
  -- New pipelines may be added at any point in the future and the source of truth for them will mostly
  -- live in the `storyteller-ml` repository.
  -- Older models and brand new models will have this field set to null, and we'll infer which value to
  -- lazily backfill based on the creation date and language setting.
  -- tts_text_pipeline_type VARCHAR(64) DEFAULT NULL,

  -- If true, multiply the mel outputs before being vocoded by a globally default constant.
  use_default_mel_multiply_factor BOOLEAN NOT NULL DEFAULT FALSE,

  -- If not null, multiply the mel outputs before being vocoded by this value.
  -- This is used instead of the default if `tts_use_default_mel_multiply_factor` is
  -- set (ie. `tts_use_default_mel_multiply_factor` is ignored and this custom value is used instead)
  maybe_custom_mel_multiply_factor DOUBLE DEFAULT NULL,

  -- If set, use a different pretrained vocoder.
  -- If not set, use the website default. (Currently HifiGan-SS)
  maybe_default_pretrained_vocoder VARCHAR(64) DEFAULT NULL,

  -- If set, we'll use a custom vocoder (from table vocoder_models) instead of a default vocoder.
  -- If not set, we'll use `maybe_default_pretrained_vocoder` (or the website default).
  maybe_custom_vocoder_token VARCHAR(32) DEFAULT NULL,


  -- ========== VECTOR CLOCK ==========

  -- Incremented with every update.
  version INT NOT NULL DEFAULT 0,


  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (model_weights_token)
  -- TODO:
  -- KEY index_model_type (model_type),
  -- KEY fk_maybe_vocoder_token (maybe_vocoder_token),
  -- KEY fk_creator_user_token (creator_user_token),
  -- KEY index_creator_set_visibility (creator_set_visibility),
  -- KEY index_private_bucket_hash (private_bucket_hash),
  -- KEY index_is_public_listing_approved (is_public_listing_approved),
  -- KEY index_is_locked_from_user_modification (is_locked_from_user_modification)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
