-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE user_subscriptions
    ADD COLUMN maybe_stripe_billing_cycle_anchor TIMESTAMP DEFAULT NULL
    AFTER maybe_stripe_invoice_is_paid;
