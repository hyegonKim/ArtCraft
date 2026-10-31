-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights
    ADD COLUMN is_in_aws BOOLEAN NOT NULL DEFAULT false
    AFTER maybe_public_bucket_extension;

ALTER TABLE model_weights
    ADD INDEX index_is_in_aws (is_in_aws);
