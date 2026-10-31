-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights
    ADD COLUMN maybe_thumbnail_token VARCHAR(32) DEFAULT NULL
    AFTER title;

ALTER TABLE model_weights
    ADD COLUMN maybe_avatar_media_file_token VARCHAR(32) DEFAULT NULL
    AFTER maybe_thumbnail_token;

CREATE INDEX fk_maybe_avatar_media_file_token ON model_weights (maybe_avatar_media_file_token);
