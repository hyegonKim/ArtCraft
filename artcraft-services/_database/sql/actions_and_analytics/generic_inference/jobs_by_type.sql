-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Pending jobs
select
    count(*),
    job_type
from generic_inference_jobs
where status in ('pending', 'started', 'attempt_failed')
group by job_type;

update generic_inference_jobs
set status = 'dead'
where status in ('pending', 'started', 'attempt_failed')

-- Histogram by type over last 50,000 jobs
-- NB: no index on ip, hence subquery
select
    job_type,
    inference_category,
    maybe_model_type,
    count(*)
FROM (
        SELECT
            job_type,
            inference_category,
            maybe_model_type
        FROM generic_inference_jobs
        ORDER BY id DESC
        LIMIT 50000
    ) as j
group by job_type, inference_category, maybe_model_type;

-- Histogram by type over last 50,000 jobs that are "still processing"
-- NB: no index on ip, hence subquery
select
    job_type,
    inference_category,
    maybe_model_type,
    count(*)
FROM (
         SELECT
             job_type,
             inference_category,
             maybe_model_type
         FROM generic_inference_jobs
         WHERE status IN ('pending', 'started', 'attempt_failed')
         ORDER BY id DESC
             LIMIT 50000
     ) as j
group by job_type, inference_category, maybe_model_type;