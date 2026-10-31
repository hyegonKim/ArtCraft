-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- All users
select count(*) from users;

-- Users registered in the last year
select count(*)
from users
where created_at > ( CURDATE() - INTERVAL 365 DAY )

-- Histogram of user signups
select
    date(created_at) as created_date,
    count(*) as registered
from users
where date(created_at) >= "2023-01-01"
group by created_date
