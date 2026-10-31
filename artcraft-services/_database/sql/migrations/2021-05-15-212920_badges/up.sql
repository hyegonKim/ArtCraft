-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Badges are rewards for loyalty
CREATE TABLE badges (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key"
  slug VARCHAR(32) NOT NULL,

  -- Description
  title VARCHAR(255) NOT NULL,
  description VARCHAR(512) NOT NULL,
  image_url VARCHAR(512) NOT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (slug)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

-- Join table
CREATE TABLE user_badges (
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  user_token VARCHAR(32) NOT NULL,
  badge_slug VARCHAR(32) NOT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  KEY fk_user_token (user_token),
  KEY fk_badge_slug (badge_slug)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
