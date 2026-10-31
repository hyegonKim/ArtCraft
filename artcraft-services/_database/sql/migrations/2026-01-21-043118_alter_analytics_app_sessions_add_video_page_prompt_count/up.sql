-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile


ALTER TABLE analytics_app_sessions
    ADD COLUMN video_page_prompt_count SMALLINT UNSIGNED NOT NULL DEFAULT 0;

