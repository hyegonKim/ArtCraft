-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- An n:m join table between prompts and media_files.
-- This associates mostly image media files with prompts, though we could conceivably
-- also store 3D sets, etc.
CREATE TABLE prompt_context_items (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- The semantic type of context.
  context_semantic_type VARCHAR(16) NOT NULL,

  -- Foreign key, many-to-many
  -- There is no uniqueness constraint for (prompt_token, media_token) because a user
  -- might attach many duplicate images to the same prompt.
  prompt_token VARCHAR(32) NOT NULL,

  -- Foreign key to the media token
  -- There is no uniqueness constraint for (prompt_token, media_token) because a user
  -- might attach many duplicate images to the same prompt.
  media_token VARCHAR(32) NOT NULL,

  -- INDICES --
  PRIMARY KEY (id),
  KEY index_prompt_token (prompt_token),
  KEY index_media_token (media_token)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
