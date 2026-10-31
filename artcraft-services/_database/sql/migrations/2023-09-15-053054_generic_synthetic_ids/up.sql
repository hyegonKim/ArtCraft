-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- It gets unwieldy to show results without a numeric sequence.
-- We create a synthetic sequence for each user to make revisiting results easier.
-- This should be a reusable table for handling this for many different types of results.
CREATE TABLE generic_synthetic_ids (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  user_token VARCHAR(32) NOT NULL,

  -- The type of synthetic id to increment.
  -- eg "media_file", "lipsync_animation", etc.
  id_category VARCHAR(32) NOT NULL,

  -- The next result id.
  -- After we use an id, we increment this counter.
  next_id BIGINT(20) NOT NULL DEFAULT 0,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (user_token, id_category),
  KEY fk_user_token (user_token)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
