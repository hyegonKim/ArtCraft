-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE users
ADD COLUMN
is_temporary BOOLEAN NOT NULL DEFAULT false;
