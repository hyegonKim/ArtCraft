-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Comments that can be attached to just about anything.
-- This is a polymorphic table and can apply to a wide variety of entities.
CREATE TABLE comments (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- Idempotency token from client
  -- This is so the frontend client doesn't submit duplicate items.
  uuid_idempotency_token VARCHAR(36) NOT NULL,

  -- User making the comment
  user_token VARCHAR(32) NOT NULL,

  -- The type of entity the comment applies to
  entity_type VARCHAR(32) NOT NULL,

  -- The token of the entity the comment applies to
  entity_token VARCHAR(32) NOT NULL,

  -- The comment in markdown
  comment_markdown TEXT NOT NULL,

  -- Generated HTML (not user-editable).
  comment_rendered_html TEXT NOT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  creator_ip_address VARCHAR(40) NOT NULL,
  editor_ip_address VARCHAR(40) NOT NULL,

  -- ========== VECTOR CLOCK ==========

  -- Incremented with every update.
  version INT NOT NULL DEFAULT 0,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- Technically, this conveys *user* edits, not things that update the record itself.
  edited_at TIMESTAMP NULL,

  -- If this is removed by a mod.
  -- It completely disappears from the system.
  user_deleted_at TIMESTAMP NULL,
  mod_deleted_at TIMESTAMP NULL,
  -- The owner of the thing the comment is attached to can delete the comment too.
  object_owner_deleted_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  UNIQUE KEY (uuid_idempotency_token),
  KEY index_entity_type_entity_token (entity_type, entity_token),
  KEY index_user_token (user_token),
  KEY index_created_at (created_at),
  KEY index_user_deleted_at (user_deleted_at),
  KEY index_mod_deleted_at (mod_deleted_at),
  KEY index_object_owner_deleted_at (object_owner_deleted_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
