-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights
    ADD COLUMN maybe_migration_old_model_token VARCHAR(32) DEFAULT NULL
    AFTER cached_user_ratings_last_updated_at;

ALTER TABLE model_weights
    ADD UNIQUE INDEX unique_index_maybe_migration_old_model_token (maybe_migration_old_model_token);
