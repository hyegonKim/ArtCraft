-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Histogram of days of content per day
select
    date(created_at) as created_date,
    sum(duration_millis) / 1000 / 60 / 60 / 24 as days
from voice_conversion_results
group by created_date

