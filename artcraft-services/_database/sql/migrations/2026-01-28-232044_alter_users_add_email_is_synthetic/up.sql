-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE users
ADD COLUMN
email_is_synthetic BOOLEAN NOT NULL DEFAULT false;
