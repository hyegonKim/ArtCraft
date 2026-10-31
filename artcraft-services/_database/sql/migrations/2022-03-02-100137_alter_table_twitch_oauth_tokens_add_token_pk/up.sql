-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- In case we ever want to query these via an admin interface, it'd be nice to not use IDs
ALTER TABLE twitch_oauth_tokens
    ADD COLUMN internal_token VARCHAR(32) NOT NULL
    AFTER id;

-- And the unique index.
ALTER TABLE twitch_oauth_tokens
    ADD UNIQUE INDEX unique_index_internal_token (internal_token);
