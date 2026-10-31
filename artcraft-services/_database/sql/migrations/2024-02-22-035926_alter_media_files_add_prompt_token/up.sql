-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files
    ADD COLUMN maybe_prompt_token VARCHAR(32) DEFAULT NULL
    AFTER maybe_text_transcript;

ALTER TABLE media_files
    ADD INDEX fk_maybe_prompt_token (maybe_prompt_token);
