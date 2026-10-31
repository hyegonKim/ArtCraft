-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE featured_items (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- ========== ENTITY LINKAGE ==========

  -- The type of record this is attached to
  -- Currently supported values:
  --   * 'model_weight'
  --   * 'media_file'
  entity_type VARCHAR(32) NOT NULL,

  -- The token for the record.
  entity_token VARCHAR(32) NOT NULL,

  -- ========== SORTING ==========

  -- A sorted ordering (optional) for pagination
  maybe_sort_order INT UNSIGNED DEFAULT NULL,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (entity_type, entity_token),

  KEY index_maybe_sort_order_asc (maybe_sort_order ASC),
  KEY index_maybe_sort_order_desc (maybe_sort_order DESC),

  KEY index_created_at (created_at),
  KEY index_deleted_at (deleted_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
