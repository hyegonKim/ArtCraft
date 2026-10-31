-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile


ALTER TABLE analytics_app_sessions
    ADD COLUMN image_to_image_count SMALLINT UNSIGNED NOT NULL DEFAULT 0;

