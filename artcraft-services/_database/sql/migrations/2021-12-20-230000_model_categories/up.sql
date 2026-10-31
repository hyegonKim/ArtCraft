-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE model_categories (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Visible "primary key"
  token VARCHAR(32) NOT NULL,

  -- Idempotency token
  -- This is so the frontend client doesn't submit duplicate requests.
  -- Only used on create.
  uuid_idempotency_token VARCHAR(36) NOT NULL,

  -- ========== MODEL TYPE ==========

  model_type ENUM(
    'not_set',
    'tts',
    'w2l'
  ) NOT NULL DEFAULT 'not_set',

  -- ========== CATEGORY TOPOLOGY ==========

  -- Optional foreign key link to super category.
  -- If absent, this is a top-level category or top-level super category.
  maybe_super_category_token VARCHAR(32) DEFAULT NULL,

  -- If this category can be applied directly to models.
  can_directly_have_models BOOLEAN NOT NULL DEFAULT false,

  -- If this category can have subcategories.
  can_have_subcategories BOOLEAN NOT NULL DEFAULT false,

  -- If only mods are allowed to tag (or untag) models with this.
  can_only_mods_apply BOOLEAN NOT NULL DEFAULT false,

  -- ========== CATEGORY NAME ==========

  -- The name of the category, which might be complicated.
  name VARCHAR(255) NOT NULL,

  -- The name displayed in the dropdown (if it should be different), otherwise NULL.
  -- eg. "Gender" (name) and "By Gender" (dropdown name)
  maybe_dropdown_name VARCHAR(20) DEFAULT NULL,

  -- ========== CREATOR DETAILS ==========

  -- The person that created the category.
  creator_user_token VARCHAR(32) NOT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  creator_ip_address_creation VARCHAR(40) NOT NULL,
  -- Technically we don't allow editing, but just in case.
  creator_ip_address_last_update VARCHAR(40) NOT NULL,

  -- ========== MODERATION DETAILS ==========

  -- Mods have to approve of categories. They remain hidden and unusable until approved.
  is_mod_approved BOOLEAN DEFAULT NULL,

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

  -- If the category is deleted and shouldn't be displayed.
  -- Only mods can delete categories.
  deleted_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  UNIQUE KEY (uuid_idempotency_token),
  KEY fk_maybe_super_category_token (maybe_super_category_token),
  KEY fk_creator_user_token (creator_user_token),
  KEY index_model_type (model_type),
  KEY index_can_directly_have_models (can_directly_have_models),
  KEY index_can_have_subcategories (can_have_subcategories),
  KEY index_is_mod_approved (is_mod_approved)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

-- Join table assigning models to zero or more categories.
CREATE TABLE tts_category_assignments (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  model_token VARCHAR(32) NOT NULL,
  category_token VARCHAR(32) NOT NULL,

  -- ========== USER DETAILS ==========

  -- The person that created the assignment.
  -- This can be the model owner or a moderator.
  category_addition_user_token VARCHAR(32) DEFAULT NULL,

  -- The person that removed the assignment.
  -- This can be the model owner or a moderator.
  category_removal_user_token VARCHAR(32) DEFAULT NULL,

  -- For abuse tracking.
  -- THIS CAN BE A MODERATOR FOR SIMPLICITY.
  -- Wide enough for IPv4/6
  ip_address_creation VARCHAR(40) NOT NULL,
  ip_address_last_update VARCHAR(40) NOT NULL,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  deleted_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (model_token, category_token),
  KEY fk_model_token (model_token),
  KEY fk_category_token (category_token),
  KEY index_deleted_at (deleted_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

-- Join table assigning models to zero or more categories.
CREATE TABLE w2l_category_assignments (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  model_token VARCHAR(32) NOT NULL,
  category_token VARCHAR(32) NOT NULL,

  -- ========== USER DETAILS ==========

  -- The person that created the assignment.
  -- This can be the model owner or a moderator.
  category_addition_user_token VARCHAR(32) DEFAULT NULL,

  -- The person that removed the assignment.
  -- This can be the model owner or a moderator.
  category_removal_user_token VARCHAR(32) DEFAULT NULL,

  -- For abuse tracking.
  -- THIS CAN BE A MODERATOR FOR SIMPLICITY.
  -- Wide enough for IPv4/6
  ip_address_creation VARCHAR(40) NOT NULL,
  ip_address_last_update VARCHAR(40) NOT NULL,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  deleted_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (model_token, category_token),
  KEY fk_model_token (model_token),
  KEY fk_category_token (category_token),
  KEY index_deleted_at (deleted_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
