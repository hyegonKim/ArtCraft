-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Statistics about any entity. Typically model weights and media files.
CREATE TABLE entity_stats (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- ========== ENTITY LINKAGE ==========

  -- The type of record this is attached to
  -- Currently supported values:
  --   * 'model_weight'
  --   * 'media_file'
  --   * 'comment'
  entity_type VARCHAR(32) NOT NULL,

  -- The token for the record.
  entity_token VARCHAR(32) NOT NULL,

  -- ========== MODEL DESCRIPTION ==========


  -- Total positive ratings
  -- Total overall ratings = positive + negative
  ratings_positive_count INT(10) UNSIGNED NOT NULL DEFAULT 0,

  -- Total negative ratings
  -- Total overall ratings = positive + negative
  ratings_negative_count INT(10) UNSIGNED NOT NULL DEFAULT 0,

  -- Total number of user bookmarks
  bookmark_count INT(10) UNSIGNED NOT NULL DEFAULT 0,

  -- Total number of views
  -- view_count INT(10) UNSIGNED NOT NULL DEFAULT 0,

  -- Total number of views
  -- play_count INT(10) UNSIGNED NOT NULL DEFAULT 0,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- When the cached ratings were last calculated and updated.
  -- maybe_last_backfilled_at TIMESTAMP DEFAULT NULL,

    -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (entity_type, entity_token),
  KEY index_entity_type (entity_type),
  KEY index_entity_token (entity_token)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
