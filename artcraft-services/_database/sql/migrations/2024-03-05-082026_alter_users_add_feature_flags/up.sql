-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE users
    ADD COLUMN maybe_feature_flags VARCHAR(255) DEFAULT NULL
    AFTER user_role_slug;
