-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- If the user has a Stripe subscription (or has had one), we link it to the user here.
-- Will never exceed 255 characters: https://groups.google.com/a/lists.stripe.com/g/api-discuss/c/1F5Wb4HRnNQ
ALTER TABLE users
    ADD COLUMN maybe_stripe_customer_id VARCHAR(255) DEFAULT NULL
    AFTER user_role_slug;
