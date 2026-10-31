-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE vocoder_models (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- The architecture of the vocoder.
  -- Currently supported values:
  --   * 'hifigan' - hifigan for Tacotron2
  --   * 'hifigan_rocket_vc' - hifigan for SoftVC ("rocket" is a codename)
  vocoder_type VARCHAR(32) NOT NULL,

  -- ========== MUTABLE METADATA ==========

  -- The "name" of the vocoder, which might be complicated.
  title VARCHAR(255) NOT NULL,

  -- The description of the model in markdown.
  description_markdown TEXT NOT NULL,

  -- Generated HTML (not user-editable).
  description_rendered_html TEXT NOT NULL,

  -- ========== MUTABLE MOD-ONLY METADATA ==========

  -- Whether the vocoder is recommended for use
  is_staff_recommended BOOLEAN NOT NULL DEFAULT FALSE,

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

  -- The "full url" of the bucket path, not including the hostname
  private_bucket_object_name VARCHAR(255) NOT NULL,

  -- ========== MODERATION DETAILS ==========

  -- Mods have locked the ability for non-authors (ie. people who didn't upload this)
  -- to assign the vocoder for use with their TTS models.
  -- TTS models that already incorporate the vocoder will not be affected.
  -- The vocoder will remain visible.
  -- This is independent of other visibility controls (is_mod_disabled_from_author_use,
  -- deleted_at, and creator_set_visibility)
  is_mod_disabled_from_public_use BOOLEAN NOT NULL DEFAULT FALSE,

  -- Mods have locked the ability for the author (ie. the person that uploaded this)
  -- to assign the vocoder for use with their TTS models.
  -- TTS models that already incorporate the vocoder will not be affected.
  -- The vocoder will remain visible.
  -- This is independent of other visibility controls (is_mod_disabled_from_public_use,
  -- deleted_at, and creator_set_visibility)
  is_mod_disabled_from_author_use BOOLEAN NOT NULL DEFAULT FALSE,

  -- Extremely popular models may be locked from deletion or modification by users.
  -- If true, only moderators can edit or delete the vocoder.
  is_mod_author_editing_locked BOOLEAN NOT NULL DEFAULT FALSE,

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
  KEY index_creator_ip_address_creation (creator_ip_address_creation),
  KEY index_creator_ip_address_last_update (creator_ip_address_last_update),
  KEY index_creator_set_visibility (creator_set_visibility),
  KEY index_private_bucket_hash (private_bucket_hash),
  KEY index_is_mod_disabled_from_public_use (is_mod_disabled_from_public_use),
  KEY index_is_mod_disabled_from_author_use (is_mod_disabled_from_author_use),
  KEY index_is_mod_author_editing_locked (is_mod_author_editing_locked)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
