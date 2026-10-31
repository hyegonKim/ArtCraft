-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE user_subscriptions CHANGE maybe_user_token user_token VARCHAR(255) NOT NULL;

ALTER TABLE user_subscriptions RENAME INDEX fk_maybe_user_token TO fk_user_token;
