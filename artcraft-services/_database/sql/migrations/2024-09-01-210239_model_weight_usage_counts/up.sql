-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Statistics on the usage of model weights.
CREATE TABLE model_weight_usage_counts (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  -- Same as model_weights.token
  token VARCHAR(32) NOT NULL,

  -- The date the usage count is for.
  -- The database timestamps are UTC, so usages within the same UTC day
  -- count towards this total.
  on_date DATE NOT NULL,

  -- The total number of usages on the date
  usage_count INT UNSIGNED NOT NULL DEFAULT 0,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token, on_date),

  KEY index_token (token),
  KEY index_on_date (on_date),
  KEY index_usage_count (usage_count)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
