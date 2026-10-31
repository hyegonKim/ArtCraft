-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE voice_conversion_models
    ADD COLUMN maybe_migration_new_model_weights_token VARCHAR(32) DEFAULT NULL
    AFTER maybe_mod_comments;

ALTER TABLE voice_conversion_models
    ADD UNIQUE INDEX unique_index_maybe_migration_new_model_weights_token (maybe_migration_new_model_weights_token);
