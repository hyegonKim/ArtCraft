-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Pending job count
select count(*) from w2l_inference_jobs where status='pending';

-- Histogram of inference
select
    date(created_at) as created_date,
    count(*) as job_count
from w2l_inference_jobs
group by created_date

-- Histogram of inference (within range)
select
    date(created_at) as created_date,
    count(*) as job_count
from w2l_inference_jobs
where date(created_at) >= "2023-01-01"
group by created_date

-- Histogram of inference (within range)
select
    date(created_at) as created_date,
    count(*) as job_count
from w2l_inference_jobs
where created_at > (CURDATE() - INTERVAL 5 DAY)
group by created_date

