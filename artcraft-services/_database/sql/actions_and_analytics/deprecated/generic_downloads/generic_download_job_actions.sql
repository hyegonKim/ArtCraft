-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- 1) Find uploads that failed (recently)

select
    id,
    token,
    download_type,
    SUBSTRING(title, 1, 35) as title,
    SUBSTRING(download_url, 1, 100) as title,
    status,
    attempt_count
from generic_download_jobs
where status IN ('started', 'attempt_failed', 'complete_failure', 'dead')
and download_type NOT IN ('vits', 'hifigan')
and created_at > NOW() - INTERVAL 30 DAY;


-- 2) Then restart them
-- NB: Double inner query is to get away from query optimizer:
--  "You can't specify target table 'generic_download_jobs' for update in FROM clause"

update generic_download_jobs
set
    status='pending',
    attempt_count=0
where token IN (
    select token from (
        select token
        from generic_download_jobs
        where status IN ('started', 'attempt_failed', 'complete_failure', 'dead')
        and download_type NOT IN ('vits', 'hifigan')
        and created_at > NOW() - INTERVAL 120 DAY
    ) as x
);


-- Kill bad pending downloads
-- eg. awful voices, like Hitler

update generic_download_jobs
set
    status='dead',
    download_url = CONCAT('NOPE', download_url)
where token = 'jdown_ssape40dxbfgvhtsmedt34418x'
limit 1;
