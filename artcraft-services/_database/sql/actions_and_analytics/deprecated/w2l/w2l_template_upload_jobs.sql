-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Get a count of pending jobs
select count(*) from w2l_template_upload_jobs where status='pending';

-- See pending download URLs:
select download_url from w2l_template_upload_jobs where status IN ('pending', 'started', 'attempt_failed');

-- See old jobs that might be stuck as "started"
select count(*)
from w2l_template_upload_jobs
where status='started'
and created_at < ( CURDATE() - INTERVAL 1 DAY );

