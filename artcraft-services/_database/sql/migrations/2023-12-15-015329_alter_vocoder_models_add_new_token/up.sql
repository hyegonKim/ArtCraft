-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE vocoder_models
    ADD COLUMN maybe_migration_new_model_weights_token VARCHAR(32) DEFAULT NULL
    AFTER maybe_mod_user_token;

ALTER TABLE vocoder_models
    ADD UNIQUE INDEX unique_index_maybe_migration_new_model_weights_token (maybe_migration_new_model_weights_token);
