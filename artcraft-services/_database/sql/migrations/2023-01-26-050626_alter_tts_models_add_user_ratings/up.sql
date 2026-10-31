-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Total count only includes "positive" and "negative" votes, not neutral ones.
ALTER TABLE tts_models
    ADD COLUMN user_ratings_total_count INT(10) UNSIGNED NOT NULL DEFAULT 0
    AFTER private_bucket_object_is_archive;

ALTER TABLE tts_models
    ADD COLUMN user_ratings_positive_count INT(10) UNSIGNED NOT NULL DEFAULT 0
    AFTER user_ratings_total_count;

ALTER TABLE tts_models
    ADD COLUMN user_ratings_negative_count INT(10) UNSIGNED NOT NULL DEFAULT 0
    AFTER user_ratings_positive_count;
