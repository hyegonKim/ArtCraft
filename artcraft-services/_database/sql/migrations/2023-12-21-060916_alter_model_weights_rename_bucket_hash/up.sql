-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights
    RENAME COLUMN private_bucket_hash TO public_bucket_hash;

ALTER TABLE model_weights
    RENAME COLUMN maybe_private_bucket_prefix TO maybe_public_bucket_prefix;

ALTER TABLE model_weights
    RENAME COLUMN maybe_private_bucket_extension TO maybe_public_bucket_extension;
