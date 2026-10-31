-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files
  ADD COLUMN is_intermediate_system_file BOOLEAN NOT NULL DEFAULT FALSE
  AFTER maybe_batch_token;

ALTER TABLE media_files
  ADD INDEX index_is_intermediate_system_file (is_intermediate_system_file);
