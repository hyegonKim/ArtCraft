-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- See how long recent jobs are taking
select
    id,
    maybe_input_source_token,
    assigned_worker,
    assigned_cluster,
    success_execution_millis / 1000 / 60 as minutes
from generic_inference_jobs
where status != 'pending'
and success_execution_millis IS NOT NULL
order by id desc
limit 50;

-- See how many are using a deleted model
select
    creator_ip_address,
    count(*)
from generic_inference_jobs
where maybe_model_token = 'vcm_4x8a30h1r26z'
and created_at >  NOW() - INTERVAL 24 HOUR
group by  creator_ip_address

-- See which deleted models are in use
select
    vm.token,
    vm.title,
    count(*)
from generic_inference_jobs as j
join voice_conversion_models as vm
on j.maybe_model_token = vm.token
where vm.mod_deleted_at IS NOT NULL
  and j.created_at >  NOW() - INTERVAL 24 HOUR
group by vm.token, vm.title

-- See who is uploading SadTalker jobs
select
    maybe_creator_user_token,
    users.username as maybe_creator_username,
    creator_ip_address,
    count(*) as attempts
from generic_inference_jobs
left outer join users
on users.token = generic_inference_jobs.maybe_creator_user_token
where maybe_model_type = 'sad_talker'
and status IN ('pending', 'started', 'complete_failure', 'attempt_failed')
group by maybe_creator_user_token, maybe_creator_username, creator_ip_address
order by attempts desc;

-- Detailed report on most recent jobs, ordered by worst performing.
-- TODO: Determine if the problem is in downloading models. Make sure the job timer doesn't include
--   sections where models / files get downloaded.
select *
from (
         select
             jobs.created_at,
             jobs.first_started_at,
             jobs.id,
             jobs.status,
             jobs.attempt_count as attempts,
             trim(replace(replace(jobs.internal_debugging_failure_reason, "\n", ""), "\r", ""))
                 as i_failure_reason,
             -- u.username,
             jobs.assigned_worker,
             jobs.assigned_cluster,
             jobs.maybe_input_source_token,
             jobs.maybe_model_type,
             jobs.maybe_model_token,
             m.media_source,
             m.maybe_original_mime_type as mime_type,
             TRUNCATE(jobs.success_execution_millis / 1000 / 60, 2) as execution_mins,
             TRUNCATE(jobs.success_inference_execution_millis / 1000 / 60, 2) as inference_mins,
             TRUNCATE((jobs.success_execution_millis - jobs.success_inference_execution_millis) / 1000 / 60, 2) as extra_mins,
             TRUNCATE(m.original_duration_millis / 1000 / 60, 2) as input_mins,
             jobs.success_execution_millis / m.original_duration_millis as ratio
         from generic_inference_jobs AS jobs
                  left join users AS u on
                 u.token = jobs.maybe_creator_user_token
                  left join media_uploads AS m on
                 m.token = jobs.maybe_input_source_token
         where
             jobs.status != 'pending'
         AND jobs.created_at > NOW() - INTERVAL 20 MINUTE
         AND jobs.maybe_model_type IN ('so_vits_svc')
         order by id desc
             limit 200
     ) as t
order by execution_mins desc;

order by assigned_worker desc, first_started_at asc;

order by first_started_at asc;

order by assigned_worker desc, execution_mins desc;


-- Get pending so-vits-svc jobs
select count(*)
from generic_inference_jobs
where maybe_model_type = 'so_vits_svc'
  and status='pending';


-- Get jobs that have routing tags
select
    id,
    maybe_input_source_token,
    maybe_creator_user_token,
    maybe_routing_tag,
    status
from generic_inference_jobs
where maybe_routing_tag IS NOT NULL
    limit 10;



-- Debug that we're setting the correct metadata on jobs
-- For some reasons storyteller-web is enqueuing the wrong type!
select
    jobs.id,
    jobs.token,
    jobs.maybe_model_type as jobs_model_type,
    jobs.maybe_model_token as jobs_model_token,
    models.token as model_token,
    models.model_type as model_type
from generic_inference_jobs as jobs
left outer join voice_conversion_models as models
on jobs.maybe_model_token = models.token
order by jobs.id
desc
limit 100;

-- Generic inference jobs + legacy TTS (this is a production endpoint)
-- UPDATED QUERY: Queues as named rows
        SELECT
            maybe_model_type as queue_type,
            count(*) as pending_job_count,
            NOW() as present_time
        FROM (
            SELECT
                token,
                maybe_model_type
            FROM generic_inference_jobs
            WHERE status IN ("pending", "attempt_failed")
            UNION
            SELECT
                token,
                maybe_model_type
            FROM generic_inference_jobs
            WHERE status IN ("started")
            AND created_at > (CURDATE() - INTERVAL 15 MINUTE)
        ) as generic_inner
        GROUP BY queue_type
UNION
        SELECT
            "legacy_tts" as queue_type,
            count(*) as pending_job_count,
            NOW() as present_time
        FROM
        (
             SELECT token
             FROM tts_inference_jobs
             WHERE status IN ("pending", "attempt_failed")
             UNION
             SELECT token
             FROM tts_inference_jobs
             WHERE status IN ("started")
             AND created_at > (CURDATE() - INTERVAL 15 MINUTE)
        ) as legacy_inner
        GROUP BY queue_type;

-- Generic inference jobs + legacy TTS (this was a production endpoint)
-- OLD QUERY: Queues as columns
SELECT
(
    SELECT
        COUNT(distinct token) as generic_job_count
    FROM
        (
            SELECT token
            FROM generic_inference_jobs
            WHERE status = "started"
              AND created_at > (CURDATE() - INTERVAL 5 MINUTE)
            UNION
            SELECT token
            FROM generic_inference_jobs
            WHERE status IN ("pending", "attempt_failed")
        ) as g
) as generic_job_count,
(
    SELECT
        COUNT(distinct token) as legacy_tts_job_count
    FROM
        (
            SELECT token
            FROM tts_inference_jobs
            WHERE status = "started"
              AND created_at > (CURDATE() - INTERVAL 5 MINUTE)
            UNION
            SELECT token
            FROM tts_inference_jobs
            WHERE status IN ("pending", "attempt_failed")
        ) as t
) as legacy_tts_job_count,
NOW() as present_time;

-- Histogram of inference
select
    date(created_at) as created_date,
    count(*) as job_count
from generic_inference_jobs
group by created_date

-- Histogram of inference (within range)
select
    date(created_at) as created_date,
    count(*) as job_count
from generic_inference_jobs
where created_at > (CURDATE() - INTERVAL 5 DAY)
group by created_date

-- Histogram of inference (within range)
select
    date(created_at) as created_date,
    count(*) as job_count
from generic_inference_jobs
where date(created_at) >= "2023-01-01"
group by created_date
