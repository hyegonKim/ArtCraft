-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights
    ADD COLUMN
         maybe_ietf_language_tag VARCHAR(64) DEFAULT NULL
    AFTER maybe_description_rendered_html;

