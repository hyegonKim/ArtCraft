-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights
    RENAME COLUMN public_bucket_hash TO private_bucket_hash;

ALTER TABLE model_weights
    RENAME COLUMN maybe_public_bucket_prefix TO maybe_private_bucket_prefix;

ALTER TABLE model_weights
    RENAME COLUMN maybe_public_bucket_extension TO maybe_private_bucket_extension;
