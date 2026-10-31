-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE user_password_resets
    RENAME COLUMN public_reset_token TO secret_key;
