-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE user_password_resets
  DROP INDEX unique_user_token_and_secret_key;

ALTER TABLE user_password_resets
  ADD UNIQUE INDEX public_reset_token (public_reset_token);

ALTER TABLE user_password_resets
  DROP INDEX index_secret_key;
