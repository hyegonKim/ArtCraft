-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE password_resets
    ADD UNIQUE INDEX unique_user_token_and_secret_key (user_token, secret_key);
