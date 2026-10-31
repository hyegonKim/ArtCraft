-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- If a user is a known-good contributor, we can assign them premium service for free.
-- Internally, our system statically maps keys to zero or more premium products.
-- The business logic can live in the plans code.
ALTER TABLE users
    ADD COLUMN maybe_loyalty_program_key VARCHAR(32) DEFAULT NULL
    AFTER maybe_stripe_customer_id;
