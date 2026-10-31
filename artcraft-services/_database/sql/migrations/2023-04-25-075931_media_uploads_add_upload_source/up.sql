-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_uploads
    ADD COLUMN media_source VARCHAR(16) NOT NULL
    AFTER media_type;
