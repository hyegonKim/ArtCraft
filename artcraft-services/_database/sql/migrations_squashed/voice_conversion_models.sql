-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE voice_conversion_models (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- ========== MODEL FUNCTIONALITY ==========

  -- The type of voice conversion model
  -- eg. 'rocket_vc' (codename for 'softvc'), 'diffsvc'
  model_type VARCHAR(32) NOT NULL,

  -- If set, we'll use the vocoder defined.
  -- If not set, some global default will be used.
  maybe_vocoder_token VARCHAR(32) DEFAULT NULL,

  -- ========== MODEL DESCRIPTION ==========

  -- The "name" of the voice model, which might be complicated.
  -- If maybe_subject_token (etc.) is set, then it's authoritative instead.
  title VARCHAR(255) NOT NULL,

  -- The description of the model in markdown.
  description_markdown TEXT NOT NULL,

  -- Generated HTML (not user-editable).
  description_rendered_html TEXT NOT NULL,

  -- The full IETF BCP47 language tag (eg. en, en-US, es-419, ja-JP, pt, etc.)
  -- (Not that it matters apart from categorization, since voice conversion is universal.)
  ietf_language_tag VARCHAR(64) NOT NULL DEFAULT 'en',

  -- The IETF BCP47 language tag's primary language subtag (eg. "es-419" becomes "es")
  -- (Not that it matters apart from categorization, since voice conversion is universal.)
  ietf_primary_language_subtag VARCHAR(12) NOT NULL DEFAULT 'en',

  -- ========== EXTRA MODEL INFO ==========

  -- RVC (v2) specific - whether the model has an index file associated with it
  -- These files improve the quality of the results.
  has_index_file BOOLEAN NOT NULL DEFAULT FALSE,

  -- ========== MODEL PLACEMENT ==========

  -- Whether the voice features on the front of FakeYou
  is_front_page_featured BOOLEAN NOT NULL DEFAULT FALSE,

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
  -- Wide enough for IPv4/6.
  -- This will pick up moderator IP addresses, but a future audit logs table
  -- will capture all edits.
  ip_address_creation VARCHAR(40) NOT NULL,
  ip_address_last_update VARCHAR(40) NOT NULL,

  -- The last user to edit the model. This could be the creator or a moderator.
  -- Check the future audit logs table for more info.
  maybe_last_update_user_token VARCHAR(32) DEFAULT NULL,

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

  -- ========== RATING AND RANKING ==========

  -- Calculated average, on a scale of 0-100
  -- Null with zero ratings.
  -- calculated_average_score INT(3) DEFAULT NULL,

  -- Other metrics
  -- calculated_total_ratings_submitted_count INT(10) NOT NULL DEFAULT 0,
  -- calculated_total_uses_count BIGINT(10) NOT NULL DEFAULT 0,

  -- ========== MODERATION DETAILS ==========

  -- Mods may have to approve of vc models for them to show up in a public index.
  -- Models can be used by the author (or other parties that know the URL) before
  -- they're approved, but unapproved models won't show up in public indices.
  -- This is independent of other visibility controls (is_mod_disabled, deleted_at,
  -- and creator_set_visibility)
  is_public_listing_approved BOOLEAN DEFAULT NULL,

  -- Extremely popular models may be locked from deletion or modification by users.
  is_locked_from_user_modification BOOLEAN NOT NULL DEFAULT FALSE,

  -- If a moderator has comments.
  maybe_mod_comments VARCHAR(255) DEFAULT NULL,

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
  KEY index_model_type (model_type),
  KEY fk_maybe_vocoder_token (maybe_vocoder_token),
  KEY fk_creator_user_token (creator_user_token),
  KEY index_creator_set_visibility (creator_set_visibility),
  KEY index_private_bucket_hash (private_bucket_hash),
  KEY index_is_public_listing_approved (is_public_listing_approved),
  KEY index_is_locked_from_user_modification (is_locked_from_user_modification)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
