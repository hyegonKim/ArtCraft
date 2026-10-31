-- NB: This is a manually squashed view of all the CREATE and ALTER statements,
-- with comments attached to the fields for centralized documentation.

-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Usages of models and assets to create things
-- This should work for inference as well as 3D object/scene creation,
-- so the inputs can be models OR files.
CREATE TABLE usages (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- ========== PRIMARY AND FOREIGN KEYS ==========

  -- Type of the entity created
  -- This only denotes which table to join against, not the type of the entity within the join table.
  -- Possible values:
  --  * 'model_weight'
  --  * 'media_file'
  --  * (maybe future types)
  created_type VARCHAR(16) NOT NULL,

  -- Token of the entity created
  created_token VARCHAR(32) NOT NULL,

  -- Type of the entity used
  -- This only denotes which table to join against, not the type of the entity within the join table.
  -- Possible values:
  --  * 'model_weight'
  --  * 'media_file'
  --  * (maybe future types)
  used_type VARCHAR(16) NOT NULL,

  -- Token of the entity used
  used_token VARCHAR(32) NOT NULL,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (created_type, created_token, used_type, used_token),
  KEY index_created_type_and_token (created_type, created_token),
  KEY index_used_type_and_token (used_type, used_token)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
