-- Find old stale jobs
select
  job_type,
  count(*)
from generic_inference_jobs
where status = 'started'
and created_at < (CURDATE() - INTERVAL 30 MINUTE)
group by job_type;

-- Kill old stale jobs
update generic_inference_jobs
set status = 'dead'
where status = 'started'
and created_at < (CURDATE() - INTERVAL 30 MINUTE)

-- Check up on recent jobs after purge
select
  job_type,
  count(*)
from generic_inference_jobs
where status = 'started'
group by job_type;

-- Kill old pending jobs too
update generic_inference_jobs
set status = 'dead'
where status = 'pending'
and created_at < (CURDATE() - INTERVAL 30 MINUTE)
