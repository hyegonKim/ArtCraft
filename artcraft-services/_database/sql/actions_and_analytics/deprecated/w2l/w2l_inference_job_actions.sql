-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Kill pending jobs
update w2l_inference_jobs set status='dead' where status IN ('pending', 'started', 'attempt_failed');

-- Unstick old jobs that might be stuck as "started"
update w2l_template_upload_jobs
set status='dead'
where status='started'
  and created_at < ( CURDATE() - INTERVAL 1 DAY );

-- Kill pending jobs with tiktok URLs:
-- These need to be blocked in the server and job directly.
update w2l_template_upload_jobs
set status='dead'
where status='pending'
  and download_url LIKE '%tiktok%';


-- Kill all pending jobs
update w2l_template_upload_jobs
set status='dead'
where status IN ('pending', 'started', 'attempt_failed');
