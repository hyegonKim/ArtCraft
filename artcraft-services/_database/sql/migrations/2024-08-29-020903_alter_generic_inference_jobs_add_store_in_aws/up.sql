-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs
    ADD COLUMN store_in_aws BOOLEAN NOT NULL DEFAULT false
    AFTER maybe_cover_image_media_file_token;
