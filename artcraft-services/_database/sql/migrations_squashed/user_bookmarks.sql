-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Favorites that can be attached to just about anything.
-- This is a polymorphic table and can apply to a wide variety of entities.
CREATE TABLE user_bookmarks
(
    -- Not used for anything except replication.
    id                      BIGINT(20)  NOT NULL AUTO_INCREMENT,

    -- Effective "primary key" (PUBLIC)
    token                   VARCHAR(32) NOT NULL,

    -- User making the favorite
    user_token              VARCHAR(32) NOT NULL,

    -- The type of entity the favorite applies to
    entity_type             VARCHAR(32) NOT NULL,

    -- The token of the entity the favorite applies to
    entity_token            VARCHAR(32) NOT NULL,

    -- ========== VECTOR CLOCK ==========

    -- Incremented with every update.
    version                 INT  NOT NULL DEFAULT 0,

    -- ========== TIMESTAMPS ==========

    created_at         TIMESTAMP   NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at         TIMESTAMP   NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    deleted_at         TIMESTAMP   NULL,

    -- INDICES --
    PRIMARY KEY (id),
    UNIQUE KEY (token),
    UNIQUE KEY (user_token, entity_type, entity_token),
    KEY index_entity_type_entity_token (entity_type, entity_token),
    KEY index_user_token_entity_token (user_token, entity_type),
    KEY index_user_token (user_token),
    KEY index_created_at (created_at),
    KEY index_user_deleted_at (deleted_at)

) ENGINE = InnoDB
  DEFAULT CHARSET = utf8mb4
  COLLATE = utf8mb4_bin;
