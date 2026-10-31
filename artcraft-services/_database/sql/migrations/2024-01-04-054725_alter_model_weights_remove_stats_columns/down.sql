-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights ADD COLUMN
cached_user_ratings_total_count INT(10) UNSIGNED NOT NULL DEFAULT 0
AFTER maybe_public_bucket_extension;

ALTER TABLE model_weights ADD COLUMN
cached_user_ratings_positive_count INT(10) UNSIGNED NOT NULL DEFAULT 0
AFTER cached_user_ratings_total_count;

ALTER TABLE model_weights ADD COLUMN
cached_user_ratings_negative_count INT(10) UNSIGNED NOT NULL DEFAULT 0
AFTER cached_user_ratings_positive_count;

ALTER TABLE model_weights ADD COLUMN
maybe_cached_user_ratings_ratio FLOAT
AFTER cached_user_ratings_negative_count;

ALTER TABLE model_weights ADD COLUMN
cached_user_ratings_last_updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
AFTER maybe_cached_user_ratings_ratio;
