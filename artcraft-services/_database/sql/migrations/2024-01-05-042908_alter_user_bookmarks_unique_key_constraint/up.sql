-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE UNIQUE INDEX unique_index_user_type_token
    ON user_bookmarks (user_token, entity_type, entity_token);