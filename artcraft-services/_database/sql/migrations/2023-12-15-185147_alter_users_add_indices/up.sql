-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE INDEX fk_maybe_avatar_media_file_token ON users (maybe_avatar_media_file_token);
CREATE INDEX fk_maybe_cover_media_file_token ON users (maybe_cover_media_file_token);
