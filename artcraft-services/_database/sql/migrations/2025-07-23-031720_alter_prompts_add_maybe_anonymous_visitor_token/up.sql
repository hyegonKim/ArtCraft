-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE prompts
    ADD COLUMN maybe_anonymous_visitor_token VARCHAR(32) DEFAULT NULL;
