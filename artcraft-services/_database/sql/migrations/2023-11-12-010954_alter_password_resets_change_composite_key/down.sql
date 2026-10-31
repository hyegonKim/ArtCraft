-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE user_password_resets
  DROP INDEX public_reset_token;

ALTER TABLE user_password_resets
  ADD UNIQUE INDEX unique_user_token_and_secret_key (user_token, public_reset_token);

ALTER TABLE user_password_resets
  ADD INDEX index_secret_key (public_reset_token);
