-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files
  ADD COLUMN is_user_upload BOOLEAN NOT NULL DEFAULT FALSE
  AFTER maybe_batch_token;

 ALTER TABLE media_files
   ADD INDEX index_media_files_is_user_upload (is_user_upload),
   ALGORITHM=INPLACE, LOCK=NONE;

