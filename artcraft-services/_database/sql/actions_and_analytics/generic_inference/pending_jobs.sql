-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Get pending job count
select count(*)
from generic_inference_jobs
where status = 'pending';
