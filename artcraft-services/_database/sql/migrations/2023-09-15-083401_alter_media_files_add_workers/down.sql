-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files DROP COLUMN generated_by_cluster;
ALTER TABLE media_files DROP COLUMN generated_by_worker;
ALTER TABLE media_files DROP COLUMN is_generated_on_prem;
