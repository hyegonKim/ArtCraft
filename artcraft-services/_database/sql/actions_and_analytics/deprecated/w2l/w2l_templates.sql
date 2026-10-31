-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Histogram of template contributions
select
    date(created_at) as created_date,
    count(*) as template_count
from w2l_templates
group by created_date

-- Histogram of template contributions (within range)
select
    date(created_at) as created_date,
    count(*) as template_count
from w2l_templates
where created_at > (CURDATE() - INTERVAL 5 DAY)
group by created_date

