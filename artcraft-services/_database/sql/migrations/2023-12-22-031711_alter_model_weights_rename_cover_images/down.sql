-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights
    RENAME COLUMN maybe_cover_image_media_file_token TO maybe_cover_media_file_token;
