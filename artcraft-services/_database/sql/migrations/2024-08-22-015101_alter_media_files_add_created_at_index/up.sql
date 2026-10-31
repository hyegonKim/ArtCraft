-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

 ALTER TABLE media_files
 ADD INDEX index_media_files_created_at (created_at),
 ALGORITHM=INPLACE, LOCK=NONE;
