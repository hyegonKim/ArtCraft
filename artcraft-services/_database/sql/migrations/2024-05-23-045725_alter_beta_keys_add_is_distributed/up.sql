-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE beta_keys
    ADD COLUMN
        is_distributed BOOLEAN NOT NULL DEFAULT FALSE
    AFTER maybe_redeemer_ip_address;
