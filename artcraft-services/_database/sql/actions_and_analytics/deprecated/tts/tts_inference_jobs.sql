-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile


-- Find jobs that are broken for a given user
select id, last_assigned_worker, internal_debugging_failure_reason,created_at
from tts_inference_jobs
where maybe_creator_user_token IN (
    select token
    from users
    where username IN ('knifecat ', 'rewritten_code', 'johnloberger')
)
  and attempt_count > 1
  and status IN ('complete_failure', 'dead', 'attempt_failed')
and created_at > ( CURDATE() - INTERVAL 5 DAY );


-- Total pending jobs
select count(*)
from tts_inference_jobs
where status = 'pending';

-- Total pending API jobs
select count(*)
from tts_inference_jobs
where status = 'pending'
and is_from_api = true;

-- Top IP addresses making requests
select distinct creator_ip_address, count(*) as attempts
from tts_inference_jobs
where status = 'pending'
group by creator_ip_address
order by attempts desc
limit 50;

-- Top (IP addresses, usernames) making requests
select distinct j.creator_ip_address, u.username, count(*) as attempts
from tts_inference_jobs AS j
left outer join users AS u
on u.token = j.maybe_creator_user_token
where j.status = 'pending'
group by j.creator_ip_address, u.username
order by attempts desc
    limit 50;

-- Find a job by an IP
select *
from tts_inference_jobs
where status = 'pending'
and creator_ip_address = '191.102.120.182'
limit 1;

-- Top voices in requests
select distinct model_token, count(*) as attempts
from tts_inference_jobs
where status = 'pending'
group by model_token
order by attempts desc
    limit 50;

-- Sample the pending inference text
select creator_ip_address,
       model_token,
       maybe_creator_user_token,
       raw_inference_text
from tts_inference_jobs
where status = 'pending';

-- Sample the pending inference text (truncated)
select creator_ip_address,
       model_token,
       maybe_creator_user_token,
       TRIM(REPLACE(SUBSTRING(raw_inference_text, 1, 50), '\n', ''))
from tts_inference_jobs
where status = 'pending'
limit 100;

-- Sample long pending texts
select creator_ip_address,
       model_token,
       maybe_creator_user_token,
       raw_inference_text
from tts_inference_jobs
where status = 'pending'
and length(raw_inference_text) > 100
limit 100;

-- Delete old TTS inference jobs (1)
DELETE FROM tts_inference_jobs
WHERE status IN ('dead', 'complete_success', 'complete_failure')
LIMIT 100000;

-- Delete old TTS inference jobs (2)
-- Roughly 20 seconds to delete 100k, 4 minutes for 1M.
DELETE FROM tts_inference_jobs
WHERE created_at < ( CURDATE() - INTERVAL 1 DAY )
LIMIT 100000;
