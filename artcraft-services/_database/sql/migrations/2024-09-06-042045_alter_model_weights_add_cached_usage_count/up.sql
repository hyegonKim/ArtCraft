-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights
    ADD COLUMN
        cached_usage_count BIGINT UNSIGNED NOT NULL DEFAULT 0
    AFTER is_not_in_gcp;

