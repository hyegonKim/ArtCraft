-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files
    ADD COLUMN media_class VARCHAR(16) NOT NULL DEFAULT 'unknown'
    AFTER media_type;

ALTER TABLE media_files
    ADD INDEX index_media_class (media_class);
