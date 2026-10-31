-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE voice_conversion_models
    DROP COLUMN maybe_migration_new_model_weights_token;
