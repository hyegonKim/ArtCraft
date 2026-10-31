-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE user_subscriptions CHANGE subscription_namespace subscription_category VARCHAR(32) NOT NULL;

ALTER TABLE user_subscriptions CHANGE subscription_product_slug subscription_product_key VARCHAR(32) NOT NULL;

ALTER TABLE user_subscriptions RENAME INDEX index_subscription_namespace TO index_subscription_category;

ALTER TABLE user_subscriptions RENAME INDEX index_subscription_product_slug TO index_subscription_product_key;
