-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights DROP COLUMN cached_user_ratings_total_count;

ALTER TABLE model_weights DROP COLUMN cached_user_ratings_positive_count;

ALTER TABLE model_weights DROP COLUMN cached_user_ratings_negative_count;

ALTER TABLE model_weights DROP COLUMN maybe_cached_user_ratings_ratio;

ALTER TABLE model_weights DROP COLUMN cached_user_ratings_last_updated_at;
