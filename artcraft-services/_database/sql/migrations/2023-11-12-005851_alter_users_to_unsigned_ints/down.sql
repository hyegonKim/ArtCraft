-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- revert
ALTER TABLE users
    MODIFY COLUMN password_version INT NOT NULL DEFAULT 0;

-- revert
ALTER TABLE users
    MODIFY COLUMN version INT NOT NULL DEFAULT 0;
