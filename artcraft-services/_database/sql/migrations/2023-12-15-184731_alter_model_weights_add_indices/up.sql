-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE INDEX index_weights_type ON model_weights (weights_type);
CREATE INDEX index_weights_category ON model_weights (weights_category);
CREATE INDEX fk_creator_user_token ON model_weights (creator_user_token);
CREATE INDEX index_creator_set_visibility ON model_weights (creator_set_visibility);
CREATE INDEX fk_maybe_avatar_media_file_token ON model_weights (maybe_avatar_media_file_token);
CREATE INDEX fk_maybe_cover_media_file_token ON model_weights (maybe_cover_media_file_token);
