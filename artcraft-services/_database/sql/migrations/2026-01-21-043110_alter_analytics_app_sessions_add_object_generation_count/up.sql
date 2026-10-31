-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile


ALTER TABLE analytics_app_sessions
    ADD COLUMN object_generation_count SMALLINT UNSIGNED NOT NULL DEFAULT 0;

