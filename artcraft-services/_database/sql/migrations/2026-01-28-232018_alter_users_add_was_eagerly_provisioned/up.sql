-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE users
ADD COLUMN
was_eagerly_provisioned BOOLEAN NOT NULL DEFAULT false;
