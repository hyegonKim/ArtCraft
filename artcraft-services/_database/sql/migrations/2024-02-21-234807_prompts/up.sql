-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Table to store prompts as a first class entity.
-- Duplication of the same prompt is totally fine.
CREATE TABLE prompts (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- ========== PRIMARY, FOREIGN KEYS, and POLYMORPHISM ==========

  -- Effective "primary key" of the prompt
  token VARCHAR(32) NOT NULL,

  -- Original product to which the prompt applied.
  -- Prompts may or may not be compatible across products.
  -- Possible values:
  --  * 'stable_diffusion'
  --  * 'comfy_ui'
  prompt_type VARCHAR(16) NOT NULL,

  -- Whomever created the prompt
  maybe_creator_user_token VARCHAR(32) DEFAULT NULL,

  -- ========== PROMPT DETAILS ==========

  -- The positive prompt (optional)
  -- TEXT = 65,535 bytes (64 KiB), ~= 4 bytes per UTF-8 character, ~ 16383 characters.
  maybe_positive_prompt TEXT DEFAULT NULL,

  -- The negative prompt (optional)
  -- TEXT = 65,535 bytes (64 KiB), ~= 4 bytes per UTF-8 character, ~ 16383 characters.
  maybe_negative_prompt TEXT DEFAULT NULL,

  -- Polymorphic arguments payload that depends on the type of inference job.
  -- TEXT = 65,535 bytes (64 KiB), ~= 4 bytes per UTF-8 character, ~ 16383 characters.
  maybe_other_args TEXT DEFAULT NULL,

  -- ========== ABUSE TRACKING ==========

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  creator_ip_address VARCHAR(40) NOT NULL,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  KEY index_maybe_creator_user_token (maybe_creator_user_token),
  KEY index_created_at (created_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
