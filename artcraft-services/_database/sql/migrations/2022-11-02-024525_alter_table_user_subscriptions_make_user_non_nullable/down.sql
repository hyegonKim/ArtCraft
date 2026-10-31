-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE user_subscriptions CHANGE user_token maybe_user_token VARCHAR(255) DEFAULT NULL;

ALTER TABLE user_subscriptions RENAME INDEX fk_user_token TO fk_maybe_user_token;
