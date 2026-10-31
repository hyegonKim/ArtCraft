-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files
    ADD COLUMN maybe_cover_image_media_file_token VARCHAR(32) DEFAULT NULL
    AFTER maybe_title;
