-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Periodically capture analytics on trending models
CREATE TABLE trending_model_analytics (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- The type of model. TTS, VC, etc.
  model_type VARCHAR(32) NOT NULL,

  -- The primary key of the TTS, VC, or other model in question.
  model_token VARCHAR(32) NOT NULL,

  -- The name of the window, eg. "last_24_hours", "last_5_hours", etc.
  window_name VARCHAR(32) NOT NULL,

  -- The measured numerical value of the statistic, eg "500 uses" = 500.
  numeric_value INT(10) NOT NULL DEFAULT 0,

  -- ========== VECTOR CLOCK ==========

  -- Incremented with every update.
  version INT NOT NULL DEFAULT 0,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (model_token, model_type, window_name),
  KEY index_model_type_model_token (model_type, model_token),
  KEY index_model_type (model_type),
  KEY index_model_token (model_token),
  KEY index_numeric_value (numeric_value)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
