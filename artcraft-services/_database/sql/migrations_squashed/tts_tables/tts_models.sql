-- NB: This is a manually squashed view of all the CREATE and ALTER statements,
-- with comments attached to the fields for centralized documentation.

-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE tts_models (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- NB: DO NOT CHANGE ORDER; APPEND ONLY!
  -- THIS MUST MATCH THE RESPECTIVE JOBS TABLE.
  tts_model_type ENUM(
      'not-set',
      'tacotron2',
      'glowtts',
      'glowtts-vocodes',
      'talknet',
      'vits'
  ) NOT NULL DEFAULT 'not-set',

  -- The name of the type of pipeline to use, eg "legacy_fakeyou", "legacy_vocodes", "english_v1", etc.
  -- New pipelines may be added at any point in the future and the source of truth for them will mostly
  -- live in the `storyteller-ml` repository.
  -- Older models and brand new models will have this field set to null, and we'll infer which value to
  -- lazily backfill based on the creation date and language setting.
  text_pipeline_type VARCHAR(64) DEFAULT NULL,

  -- For models distributed as archives/bundles (eg. TalkNet), we can pack
  -- in a vocoder. Note that this vocoder does not have to be used.
  has_self_contained_vocoder BOOLEAN NOT NULL DEFAULT FALSE,

  -- For models distributed as archives/bundles (eg. TalkNet), we can pack
  -- in a duration model.
  has_self_contained_duration_model BOOLEAN NOT NULL DEFAULT FALSE,

  -- For models distributed as archives/bundles (eg. TalkNet), we can pack
  -- in a pitch model.
  has_self_contained_pitch_model BOOLEAN NOT NULL DEFAULT FALSE,

  -- NB(2022-07-05): THIS IS NOW MEANINGLESS AND SUPERSEDED BY `TEXT_PIPELINE_TYPE`.
  -- NB(2024-01-20): All 8720 records in the database are "basic".
  -- NB: DO NOT CHANGE ORDER; APPEND ONLY!
  -- How text should be handled.
  text_preprocessing_algorithm ENUM(
      'basic',
      'arpabet'
  ) NOT NULL DEFAULT 'basic',

  -- If true, multiply the mel outputs before being vocoded by a globally default constant.
  use_default_mel_multiply_factor BOOLEAN NOT NULL DEFAULT FALSE,

  -- If not null, multiply the mel outputs before being vocoded by this value.
  -- This is used instead of the default if `use_default_mel_multiply_factor` is
  -- set (ie. `use_default_mel_multiply_factor` is ignored and this custom value is used instead)
  maybe_custom_mel_multiply_factor DOUBLE DEFAULT NULL,

  -- If set, use a different pretrained vocoder.
  -- If not set, use the website default. (Currently HifiGan-SS)
  maybe_default_pretrained_vocoder VARCHAR(64) DEFAULT NULL,

  -- If set, we'll use a custom vocoder (from table vocoder_models) instead of a default vocoder.
  -- If not set, we'll use `maybe_default_pretrained_vocoder` (or the website default).
  maybe_custom_vocoder_token VARCHAR(32) DEFAULT NULL,

  -- Optional Pointer to a newer version of the voice
  -- If there's a newer version, we can disable this one.
  -- maybe_updated_model_token VARCHAR(32) DEFAULT NULL,

  -- We can set as a combination of ['username' + 'voice-name']
  -- There can be public aliases for voices, eg. a voice's default model.
  -- A user can change this.
  -- As such, these should not be foreign keys.
  -- updatable_slug VARCHAR(64) NOT NULL,

  -- Can be linked to a well-known voice
  -- maybe_subject_token VARCHAR(32) DEFAULT NULL,
  -- maybe_actor_subject_token VARCHAR(32) DEFAULT NULL,

  -- ========== UNSTRUCTURED METADATA ==========

  -- The full IETF BCP47 language tag (eg. en, en-US, es-419, ja-JP, pt, etc.)
  -- The wide varchar space is because these language tags can become cumbersome, eg "hy-Latn-IT-arevela"
  ietf_language_tag VARCHAR(64) NOT NULL DEFAULT 'en',

  -- The IETF BCP47 language tag's primary language subtag (eg. "es-419" becomes "es" and "en-US" becomes "en")
  ietf_primary_language_subtag VARCHAR(12) NOT NULL DEFAULT 'en',

  -- The "name" of the voice model, which might be complicated.
  -- If maybe_subject_token (etc.) is set, then it's authoritative instead.
  title VARCHAR(255) NOT NULL,

  -- The description of the model in markdown.
  description_markdown TEXT NOT NULL,

  -- Generated HTML (not user-editable).
  description_rendered_html TEXT NOT NULL,

  -- eg. Bill Clinton, Batman, or Goku.
  voice_name VARCHAR(255) DEFAULT NULL,

  -- Optional; eg. Sean Schemmel
  actor_name VARCHAR(255) DEFAULT NULL,

  -- Optional; eg. if the voice is "happy" or a singer "a-capella", etc.
  characteristic VARCHAR(255) DEFAULT NULL,

  -- Optional; eg. if you train the model more than once.
  version_string VARCHAR(255) DEFAULT NULL,

  -- Whether the voice features on the front of FakeYou or Storyteller Twitch TTS
  is_front_page_featured BOOLEAN NOT NULL DEFAULT FALSE,
  is_twitch_featured BOOLEAN NOT NULL DEFAULT FALSE,

  -- A suggested bot command prefix, eg. 'sonic' that could be interpreted as '/sonic'
  -- by a TTS system. These are unique strings that can only be applied to one model at
  -- a time at most. Only moderators can set this, and it's only for the best voices.
  maybe_suggested_unique_bot_command VARCHAR(255) DEFAULT NULL,

  -- ========== IMMUTABLE PROVENANCE AND METADATA ==========

  -- Where the file was originally downloaded (if it was downloaded)
  original_download_url VARCHAR(512) DEFAULT NULL,

  -- The filename that was used at upload time (if available)
  original_filename VARCHAR(255) DEFAULT NULL,

  -- File characteristics
  file_size_bytes INT(10) NOT NULL DEFAULT 0,

  -- ========== CREATOR DETAILS ==========

  -- The person that created the template.
  creator_user_token VARCHAR(32) NOT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  creator_ip_address_creation VARCHAR(40) NOT NULL,
  creator_ip_address_last_update VARCHAR(40) NOT NULL,

  -- (THIS MIGHT NOT BE USED)
  -- NB: DO NOT SORT!
  -- THIS MUST MATCH THE RESPECTIVE JOBS TABLE.
  creator_set_visibility ENUM(
    'public',
    'hidden',
    'private'
   ) NOT NULL DEFAULT 'public',

  -- ========== BUCKET STORAGE ==========

  -- The pytorch model
  -- For now, this will be a hash of the file contents.
  -- NB: NOT UNIQUE! We can allow duplicate uploads.
  private_bucket_hash CHAR(64) NOT NULL,
  -- The "full url" version of the path
  private_bucket_object_name VARCHAR(255) NOT NULL,

  -- Whether the file is a zip archive (rather than a model file).
  private_bucket_object_is_archive BOOLEAN NOT NULL DEFAULT FALSE,

  -- ========== RATING AND RANKING ==========

  -- Calculated average, on a scale of 0-100
  -- Null with zero ratings.
  -- calculated_average_score INT(3) DEFAULT NULL,

  -- Other metrics
  -- calculated_total_ratings_submitted_count INT(10) NOT NULL DEFAULT 0,
  -- calculated_total_uses_count BIGINT(10) NOT NULL DEFAULT 0,

  -- Total count only includes "positive" and "negative" votes, not neutral ones.
  user_ratings_total_count INT(10) UNSIGNED NOT NULL DEFAULT 0,
  user_ratings_positive_count INT(10) UNSIGNED NOT NULL DEFAULT 0,
  user_ratings_negative_count INT(10) UNSIGNED NOT NULL DEFAULT 0,

  -- ========== MODERATION DETAILS ==========

  -- Mods have to approve of tts models for them to show up in a public index.
  -- Models can be used by the author (or other parties that know the URL) before
  -- they're approved, but unapproved models won't show up in public indices.
  -- This is independent of other visibility controls (is_mod_disabled, deleted_at,
  -- and creator_set_visibility)
  is_public_listing_approved BOOLEAN DEFAULT NULL,

  -- Extremely popular models may be locked from deletion or modification by users.
  is_locked_from_user_modification BOOLEAN NOT NULL DEFAULT FALSE,

  -- In this case, a moderator disables it from being used for inference.
  -- This also disables it for the creator.
  -- Unlike deletion, it remains "visible" to those that have access.
  is_locked_from_use BOOLEAN NOT NULL DEFAULT FALSE,

  -- If a moderator has comments.
  maybe_mod_comments VARCHAR(255) DEFAULT NULL,

  -- The last moderator that made changes.
  maybe_mod_user_token VARCHAR(32) DEFAULT NULL,

  -- ========== MIGRATION DETAILS ==========

  -- If migrated to model_weights, this is the new token
  maybe_migration_new_model_weights_token VARCHAR(32) DEFAULT NULL,

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
  KEY fk_creator_user_token (creator_user_token),
  KEY fk_maybe_mod_user_token (maybe_mod_user_token),
  KEY fk_maybe_custom_vocoder_token (maybe_custom_vocoder_token),
  KEY index_creator_ip_address_creation (creator_ip_address_creation),
  KEY index_creator_ip_address_last_update (creator_ip_address_last_update),
  KEY index_creator_set_visibility (creator_set_visibility),
  KEY index_private_bucket_hash (private_bucket_hash),
  KEY index_is_public_listing_approved (is_public_listing_approved),
  KEY index_is_locked_from_user_modification (is_locked_from_user_modification),
  KEY index_is_locked_from_use (is_locked_from_use),
  KEY index_tts_model_type (tts_model_type)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
