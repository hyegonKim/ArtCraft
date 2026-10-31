-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- This finds the top users of image generation within a given time range.


select
    u.username,
    u.email_address,
    count(*) as use_count
from users as u
join
(
  select maybe_creator_user_token as user_token
    from generic_inference_jobs
    where job_type = 'stable_diffusion'
      AND created_at > NOW() - INTERVAL 24 HOUR
) as jobs
ON jobs.user_token = u.token
group by u.username, u.email_address
order by use_count desc
