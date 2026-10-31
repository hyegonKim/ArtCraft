-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Drop column
ALTER TABLE model_weights DROP COLUMN maybe_thumbnail_token;
ALTER TABLE model_weights DROP COLUMN maybe_avatar_media_file_token;
