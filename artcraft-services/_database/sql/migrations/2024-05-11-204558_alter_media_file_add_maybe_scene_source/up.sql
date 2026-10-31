-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files
  ADD COLUMN maybe_scene_source_media_file_token VARCHAR(32) DEFAULT NULL
  AFTER maybe_style_transfer_source_media_file_token;

ALTER TABLE media_files
  ADD INDEX index_maybe_scene_source_media_file_token (maybe_scene_source_media_file_token);
