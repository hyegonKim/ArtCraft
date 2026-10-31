-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE beta_keys
  ADD COLUMN maybe_expires_at TIMESTAMP NULL
  AFTER created_at;

