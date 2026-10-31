-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

 ALTER TABLE generic_inference_jobs
 ADD INDEX index_maybe_creator_anonymous_visitor_token (maybe_creator_anonymous_visitor_token),
 ALGORITHM=INPLACE, LOCK=NONE;
