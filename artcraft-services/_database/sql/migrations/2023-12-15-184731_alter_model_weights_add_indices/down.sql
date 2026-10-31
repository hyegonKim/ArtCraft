-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

DROP INDEX index_weights_type ON model_weights;
DROP INDEX index_weights_category ON model_weights;
DROP INDEX fk_creator_user_token ON model_weights;
DROP INDEX index_creator_set_visibility ON model_weights;
DROP INDEX fk_maybe_avatar_media_file_token ON model_weights;
DROP INDEX fk_maybe_cover_media_file_token ON model_weights;
