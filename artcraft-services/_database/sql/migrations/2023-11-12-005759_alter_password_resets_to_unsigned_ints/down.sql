-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- revert
ALTER TABLE user_password_resets
    MODIFY COLUMN version INT NOT NULL DEFAULT 0;

-- revert
ALTER TABLE user_password_resets
    MODIFY COLUMN current_password_version INT NOT NULL DEFAULT 0;
