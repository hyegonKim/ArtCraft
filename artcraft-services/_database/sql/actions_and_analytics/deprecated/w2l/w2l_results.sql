-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Histogram of results
select
    date(created_at) as created_date,
    count(*) as result_count
from w2l_results
group by created_date

-- Histogram of results and durations
select
    date(created_at) as created_date,
    sum(duration_millis) / 1000 / 60 / 60 as hours,
    avg(duration_millis) / 60 as average_seconds,
    count(*) as video_count
from w2l_results
group by created_date

-- Histogram of results (within range)
select
    date(created_at) as created_date,
    count(*) as result_count
from w2l_results
where created_at > (CURDATE() - INTERVAL 5 DAY)
group by created_date

-- Created by date (within range)
select
    date(created_at) as created_date,
    count(*) as use_count
from w2l_results
where date(created_at) > "2023-07-31"
group by created_date

select
    date(created_at) as created_date,
    count(*) as created_count
from tts_inference_jobs
where created_at > (CURDATE() - INTERVAL 90 DAY)
group by created_date
