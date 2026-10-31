-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE beta_keys
  ADD COLUMN
    creator_user_token VARCHAR(32) NOT NULL
  AFTER key_value;

ALTER TABLE beta_keys
  ADD INDEX index_creator_user_token (creator_user_token);
