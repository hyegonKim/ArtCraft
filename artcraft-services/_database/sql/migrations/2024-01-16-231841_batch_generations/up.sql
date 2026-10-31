-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Batch generations
CREATE TABLE batch_generations (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- The batch token
  -- This is **NOT** a unique token.
  token VARCHAR(32) NOT NULL,

  -- ========== ENTITY LINKAGE ==========

  -- The type of record this is attached to
  -- Currently supported values:
  --   * 'model_weight'
  --   * 'media_file'
  --   * 'comment'
  entity_type VARCHAR(32) NOT NULL,

  -- The token for the record.
  entity_token VARCHAR(32) NOT NULL,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token, entity_type, entity_token),
  KEY index_token (token),
  KEY index_entity_token (entity_token)

  -- NB: Realistically, we probably don't need this index until we have a lot of entity types, and the entity_token
  -- entropy is probably sufficient enough as-is.
  -- KEY index_entity_type_and_token (entity_type, entity_token),


) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
