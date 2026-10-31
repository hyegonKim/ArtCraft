-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

 ALTER TABLE generic_inference_jobs
 ADD INDEX index_generic_inference_jobs_created_at (created_at),
 ALGORITHM=INPLACE, LOCK=NONE;

-- Inserts into production database:
-- INSERT INTO __diesel_schema_migrations (version, run_on)
-- VALUES
-- ("20240822015101", "2024-08-22 23:20:31"),
-- ("20240822015117", "2024-08-22 23:20:32"),
-- ("20240822015156", "2024-08-22 23:20:32");

