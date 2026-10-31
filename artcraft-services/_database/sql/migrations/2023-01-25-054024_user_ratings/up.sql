-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- User ratings of different types of content
-- This is a polymorphic table and can apply to a wide variety of entities.
CREATE TABLE user_ratings (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- User making the rating
  user_token VARCHAR(32) NOT NULL,

  -- The type of entity mutated
  entity_type VARCHAR(32) NOT NULL,

  -- The token of the entity mutated
  entity_token VARCHAR(32) NOT NULL,

  -- Whether the vote is up/positive (TRUE) or down/negative (FALSE)
  -- is_positive_rating BOOLEAN NOT NULL DEFAULT FALSE,

  -- Whether the vote is "soft deleted" (="neutral"), up/positive, or down/negative.
  -- Rather than a nullable bool for ratings, we'll provide an enum.
  rating_value ENUM(
    'neutral',
    'positive',
    'negative'
  ) NOT NULL DEFAULT 'neutral',

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  vote_ip_address VARCHAR(40) NOT NULL,

  -- ========== VECTOR CLOCK ==========

  -- Incremented with every update.
  version INT NOT NULL DEFAULT 0,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (user_token, entity_type, entity_token),
  KEY index_entity_type_entity_token (entity_type, entity_token),
  KEY index_user_token (user_token),
  KEY index_rating_value (rating_value)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
