-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE user_subscriptions
    ADD COLUMN maybe_stripe_invoice_is_paid BOOLEAN DEFAULT NULL
    AFTER maybe_stripe_subscription_status;
