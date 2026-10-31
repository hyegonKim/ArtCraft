-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- NB: See the "squashed" copy of this for better documentation
-- and the current state of the table.

CREATE TABLE tts_models (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- NB: THIS ENUM IS OUT OF DATE!
  -- Check the squashed sql file as an updated reference!
  -- NB: DO NOT CHANGE ORDER; APPEND ONLY!
  -- THIS MUST MATCH THE RESPECTIVE JOBS TABLE.
  tts_model_type ENUM(
      'not-set',
      'tacotron2',
      'glowtts',
      'glowtts-vocodes'
  ) NOT NULL DEFAULT 'not-set',

  -- NB: DO NOT CHANGE ORDER; APPEND ONLY!
  -- How text should be handled.
  text_preprocessing_algorithm ENUM(
      'basic',
      'arpabet'
  ) NOT NULL DEFAULT 'basic',

  -- NB: ADDED BY ALTER
  -- maybe_default_pretrained_vocoder VARCHAR(64) DEFAULT NULL,

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

  -- ========== RATING AND RANKING ==========

  -- Calculated average, on a scale of 0-100
  -- Null with zero ratings.
  -- calculated_average_score INT(3) DEFAULT NULL,

  -- Other metrics
  -- calculated_total_ratings_submitted_count INT(10) NOT NULL DEFAULT 0,
  -- calculated_total_uses_count BIGINT(10) NOT NULL DEFAULT 0,

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
  KEY index_is_public_listing_approved (is_public_listing_approved),
  KEY index_is_locked_from_user_modification (is_locked_from_user_modification),
  KEY index_is_locked_from_use (is_locked_from_use)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

CREATE TABLE w2l_templates (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- NB: DO NOT CHANGE ORDER; APPEND ONLY!
  -- THIS MUST MATCH THE RESPECTIVE JOBS TABLE.
  template_type ENUM(
    'not-set',
    'image',
    'video'
  ) NOT NULL DEFAULT 'not-set',

  -- Optional Pointer to a newer version of the template
  -- If there's a newer version, we can disable this one.
  -- maybe_updated_template_token VARCHAR(32) DEFAULT NULL,

  -- A combination of ['username' + 'template-name']
  -- There can be public aliases for voices, eg. a voice's default model.
  -- A user can change this.
  -- As such, these should not be foreign keys.
  -- updatable_slug VARCHAR(64) NOT NULL,

  -- Can be linked to a well-known voice
  -- maybe_subject_token VARCHAR(32) DEFAULT NULL,
  -- maybe_actor_subject_token VARCHAR(32) DEFAULT NULL,

  -- ========== UNSTRUCTURED METADATA ==========

  -- The title of the template.
  title CHAR(255) NOT NULL,

  -- The description of the template in markdown.
  description_markdown TEXT NOT NULL,

  -- Generated HTML (not user-editable).
  description_rendered_html TEXT NOT NULL,

  -- ========== IMMUTABLE PROVENANCE AND METADATA ==========

  -- Where the file was originally downloaded (if it was downloaded)
  original_download_url VARCHAR(512) DEFAULT NULL,

  -- The filename that was used at upload time (if available)
  original_filename CHAR(255) DEFAULT NULL,

  -- Dimensions and other characteristics
  file_size_bytes INT(10) NOT NULL DEFAULT 0,
  mime_type VARCHAR(32) NOT NULL DEFAULT '',
  frame_width INT(10) NOT NULL DEFAULT 0,
  frame_height INT(10) NOT NULL DEFAULT 0,
  frame_count INT(10) NOT NULL DEFAULT 0,
  fps FLOAT NOT NULL DEFAULT 0.0,
  duration_millis INT(10) NOT NULL DEFAULT 0,

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

  -- The hash of the original source image/video
  private_bucket_hash CHAR(64) NOT NULL,

  -- The "full path" version of the path. Contains the hash.
  private_bucket_object_name VARCHAR(255) NOT NULL,
  -- The "full path" of the cached faces object. Contains the hash.
  private_bucket_cached_faces_object_name VARCHAR(255) NOT NULL,

  -- The "full url" of the preview image and/or video. Contains the hash.
  maybe_public_bucket_preview_image_object_name VARCHAR(255) DEFAULT NULL,
  maybe_public_bucket_preview_video_object_name VARCHAR(255) DEFAULT NULL,

  -- ========== RATING AND RANKING ==========

  -- Calculated average, on a scale of 0-100
  -- Null with zero ratings.
  -- calculated_average_score INT(3) DEFAULT NULL,

  -- Other metrics
  -- calculated_total_ratings_submitted_count INT(10) NOT NULL DEFAULT 0,
  -- calculated_total_uses_count BIGINT(10) NOT NULL DEFAULT 0,

  -- ========== MODERATION DETAILS ==========

  -- Mods have to approve of w2l templates for them to show up in a public index.
  -- Templates can be used by the author (or other parties that know the URL) before
  -- they're approved, but unapproved templates won't show up in public indices.
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

  -- ========== VECTOR CLOCK ==========

  -- Incremented with every update.
  version INT NOT NULL DEFAULT 0,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- If this is removed by a mod.
  -- It shows up nowhere if deleted.
  user_deleted_at TIMESTAMP NULL,
  mod_deleted_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  KEY fk_creator_user_token (creator_user_token),
  KEY fk_maybe_mod_user_token (maybe_mod_user_token),
  KEY index_template_type (template_type),
  KEY index_creator_ip_address_creation (creator_ip_address_creation),
  KEY index_creator_ip_address_last_update (creator_ip_address_last_update),
  KEY index_creator_set_visibility (creator_set_visibility),
  KEY index_private_bucket_hash (private_bucket_hash),
  KEY index_is_public_listing_approved (is_public_listing_approved),
  KEY index_is_locked_from_user_modification (is_locked_from_user_modification),
  KEY index_is_locked_from_use (is_locked_from_use)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
