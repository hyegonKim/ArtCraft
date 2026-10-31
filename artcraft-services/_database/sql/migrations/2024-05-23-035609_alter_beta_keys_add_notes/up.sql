-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE beta_keys
  ADD COLUMN maybe_notes VARCHAR(255) DEFAULT NULL
  AFTER maybe_redeemer_ip_address;

