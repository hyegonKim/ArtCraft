-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- This table describes a zero shot voice in the process of being created (or later reused / edited / enhanced)
-- These are not usable voices until their vector embeddings have been created, and this table does not serve
-- the production voice list.
--
-- Multiple voices can be created from a single "dataset" as the data set is mutable over time. Users can add or
-- remove new samples to the data set at any time.
CREATE TABLE zs_voice_datasets (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" for joins, API, etc. (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- NB: No idempotency token.
  -- Store it in another table to prevent duplicate record creation from rapid user inputs.

  -- ========== VOICE DETAILS ==========

  -- The "title" of the voice. This can be the name and additional information as supplied by the user.
  -- When the voice embeddings are created, this will be copied to the produced "voice" (embeddings) record.
  title VARCHAR(255) NOT NULL,

  -- The full IETF BCP47 language tag (eg. en, en-US, es-419, ja-JP, pt, etc.)
  -- The wide varchar space is because these language tags can become cumbersome, eg "hy-Latn-IT-arevela"
  ietf_language_tag VARCHAR(64) NOT NULL DEFAULT 'en',

  -- The IETF BCP47 language tag's primary language subtag (eg. "es-419" becomes "es" and "en-US" becomes "en")
  ietf_primary_language_subtag VARCHAR(12) NOT NULL DEFAULT 'en',

  -- TODO: misc speaker metadata
  -- We'll want rich metadata for search
  -- When the voice embeddings are created, this will be copied to the produced "voice" (embeddings) record.
  -- We don't want a hard link between the dataset and the produced voice as the voice is static once
  -- created (with the exception of regenerating embeddings), but the dataset my drift as the user experiments,
  -- adds/removes samples, etc.
  -- series_name VARCHAR(255) DEFAULT NULL,
  -- speaker_name VARCHAR(255) DEFAULT NULL,
  -- speaker_gender VARCHAR(255) DEFAULT NULL,
  -- speaker_age VARCHAR(255) DEFAULT NULL,
  -- speaker_voice_style VARCHAR(255) DEFAULT NULL,

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

  KEY index_ietf_primary_language_subtag (ietf_primary_language_subtag),
  KEY fk_maybe_creator_user_token (maybe_creator_user_token),
  KEY fk_maybe_mod_user_token (maybe_mod_user_token),
  KEY index_creator_ip_address (creator_ip_address),
  KEY index_user_deleted_at (user_deleted_at),
  KEY index_mod_deleted_at (mod_deleted_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
