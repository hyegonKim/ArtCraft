-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- This table contains both zero shot TTS voices (speaker embeddings) and zero shot V2V voices (speaker embeddings).
-- These are the zero shot voices that are usable by our users.
CREATE TABLE zs_voices (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" for joins, API, etc. (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- NB: No idempotency token.
  -- Store it in another table to prevent duplicate record creation from rapid user inputs.

  -- ========== ZERO SHOT FOREIGN KEYS AND STATE==========

  -- The zero shot dataset that was used to create this voice.
  -- The dataset will only be accessible to the original author of the voice.
  -- The original author can create multiple voices from the same dataset.
  dataset_token VARCHAR(32) NOT NULL,

  -- When datasets are edited, we bump their version.
  -- This tells us which version of the dataset created this voice (not that we should preserve
  -- record of which samples are in which version.)
  -- We can surface this to the users as well.
  dataset_version INT NOT NULL DEFAULT 0,

  -- ========== MODEL DETAILS ==========

  -- The class or category of model/algorithm this voice is intended for.
  --   * 'tts' for zero shot TTS
  --   * 'vc' for zero shot voice conversion
  --   * 'pitch' for pitch correction models (hypothetical)
  --   * (Since this isn't an enum, the list of values can be expanded.)
  model_category VARCHAR(16) NOT NULL,

  -- The model used for the voice:
  --   * 'vall-e-x' for Vall-E-X
  --   * 'ns2vc' for NaturalSpeech2 voice conversion (hypothetical)
  --   * (Since this isn't an enum, the list of values can be expanded.)
  model_type VARCHAR(16) NOT NULL,

  -- The version of the model.
  -- If we deploy new model weights for the model, we may need to regenerate
  -- all of the corresponding speaker embeddings. This lets us know which voices
  -- are the old version.
  -- In some cases, we may need to preserve, hide, or roll back to older versions.
  model_version INTEGER UNSIGNED NOT NULL DEFAULT 0,

  -- What kind of features these are: spectrogram, encodec, etc.
  -- Technically we shouldn't need this, as the 2-tuple (model_type, model_version) implicitly knows
  -- what its features should be, but this is probably useful for our own analysis and may be easier
  -- to deal with.
  --
  -- Example values:
  --   * 'spectrogram'
  --   * 'encodec'
  --   * (Since this isn't an enum, the list of values can be expanded.)
  encoding_type VARCHAR(16) NOT NULL,

  -- ========== VOICE DETAILS ==========

  -- The "title" of the voice. This can be the name and additional information as supplied by the user.
  title VARCHAR(255) NOT NULL,

  -- The full IETF BCP47 language tag (eg. en, en-US, es-419, ja-JP, pt, etc.)
  -- The wide varchar space is because these language tags can become cumbersome, eg "hy-Latn-IT-arevela"
  ietf_language_tag VARCHAR(64) NOT NULL DEFAULT 'en',

  -- The IETF BCP47 language tag's primary language subtag (eg. "es-419" becomes "es" and "en-US" becomes "en")
  ietf_primary_language_subtag VARCHAR(12) NOT NULL DEFAULT 'en',

  -- TODO: misc speaker metadata
  -- We'll want rich metadata for search
  -- series_name VARCHAR(255) DEFAULT NULL,
  -- speaker_name VARCHAR(255) DEFAULT NULL,
  -- speaker_gender VARCHAR(255) DEFAULT NULL,
  -- speaker_age VARCHAR(255) DEFAULT NULL,
  -- speaker_voice_style VARCHAR(255) DEFAULT NULL,

  -- ========== STORAGE DETAILS ==========

  -- The hash for the bucket directory that contains the original upload
  -- eg. /speaker_vectors/{bucket_hash}/{voice_model_version}.bin
  bucket_hash  VARCHAR(32) NOT NULL,

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

  -- TODO: These are not correct. Fix these before landing / running migrations.
  KEY index_model_category (model_category),
  KEY index_model_type (model_type),
  KEY index_model_type_and_version (model_type, model_version),
  KEY index_encoding_type (encoding_type),
  KEY index_ietf_primary_language_subtag (ietf_primary_language_subtag),
  KEY fk_maybe_creator_user_token (maybe_creator_user_token),
  KEY fk_maybe_mod_user_token (maybe_mod_user_token),
  KEY index_creator_ip_address (creator_ip_address),
  KEY index_user_deleted_at (user_deleted_at),
  KEY index_mod_deleted_at (mod_deleted_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
