-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Every type of fine tuned model that users can upload.
CREATE TABLE model_weights_extension_image_generation_details (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Simultaneous primary and foreign key.
  -- This should match a single record in the `model_weights` table.
  -- This turns this table into a polymorphic extension table.
  model_weights_token VARCHAR(32) NOT NULL,


  -- ========== IMAGE GENERATION MODEL METADATA ==========

  -- RVC (v2) specific - whether the model has an index file associated with it
  -- These files improve the quality of the results.
  vc_has_index_file BOOLEAN NOT NULL DEFAULT FALSE,

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
