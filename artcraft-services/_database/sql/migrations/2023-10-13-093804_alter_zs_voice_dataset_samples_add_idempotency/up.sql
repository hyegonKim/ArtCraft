-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE zs_voice_dataset_samples
    ADD COLUMN uuid_idempotency_token VARCHAR(36) NOT NULL
    AFTER token;

ALTER TABLE zs_voice_dataset_samples
    ADD UNIQUE INDEX unique_index_uuid_idempotency_token (uuid_idempotency_token);
