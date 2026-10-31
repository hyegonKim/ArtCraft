-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files
    ADD COLUMN is_generated_on_prem BOOLEAN NOT NULL DEFAULT FALSE
    AFTER maybe_mod_user_token;

ALTER TABLE media_files
    ADD COLUMN generated_by_worker VARCHAR(255) DEFAULT NULL
    AFTER is_generated_on_prem;

ALTER TABLE media_files
    ADD COLUMN generated_by_cluster VARCHAR(255) DEFAULT NULL
    AFTER generated_by_worker;
