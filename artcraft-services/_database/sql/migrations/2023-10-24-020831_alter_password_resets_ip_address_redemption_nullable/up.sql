-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE password_resets
    MODIFY ip_address_redemption VARCHAR(40) DEFAULT NULL;
