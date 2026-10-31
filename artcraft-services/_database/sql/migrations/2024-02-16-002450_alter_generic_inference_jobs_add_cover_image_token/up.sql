-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs
    ADD COLUMN maybe_cover_image_media_file_token VARCHAR(32) DEFAULT NULL
    AFTER maybe_download_url;
