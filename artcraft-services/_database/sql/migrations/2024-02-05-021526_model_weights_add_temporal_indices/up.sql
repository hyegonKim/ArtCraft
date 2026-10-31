-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE INDEX index_updated_at ON model_weights (updated_at);
CREATE INDEX index_user_deleted_at ON model_weights (user_deleted_at);
CREATE INDEX index_mod_deleted_at ON model_weights (mod_deleted_at);
