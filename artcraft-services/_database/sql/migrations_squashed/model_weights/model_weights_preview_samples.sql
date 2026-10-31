-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE model_weights_preview_samples (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- ========== WEIGHTS CLASSIFICATION ==========

  -- Foreign key to model weights.
  -- This is in a composite index with media_file_token to make sure a media file only applies
  -- to a given model once.
  model_weight_token VARCHAR(32) NOT NULL,

  -- Foreign key to media files
  -- This is in a composite index with model_weight_token to make sure a media file only applies
  -- to a given model once.
  media_file_token VARCHAR(32) NOT NULL,

  -- Information about who set the sample
  --  * automation
  --  * author
  --  * community
  set_by VARCHAR(16) NOT NULL,

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
  UNIQUE KEY (model_weight_token, media_file_token),

  KEY index_model_weight_token (model_weight_token),
  KEY index_media_file_token (media_file_token)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
