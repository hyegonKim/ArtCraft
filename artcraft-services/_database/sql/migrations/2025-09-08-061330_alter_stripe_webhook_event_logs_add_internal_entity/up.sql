-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE stripe_webhook_event_logs
    ADD COLUMN maybe_internal_entity_token VARCHAR(32) DEFAULT NULL
    AFTER maybe_user_token;
