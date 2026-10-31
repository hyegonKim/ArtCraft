-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE user_subscriptions CHANGE subscription_category subscription_namespace VARCHAR(32) NOT NULL;

ALTER TABLE user_subscriptions CHANGE subscription_product_key subscription_product_slug VARCHAR(32) NOT NULL;

ALTER TABLE user_subscriptions RENAME INDEX index_subscription_category TO index_subscription_namespace;

ALTER TABLE user_subscriptions RENAME INDEX index_subscription_product_key TO index_subscription_product_slug;
