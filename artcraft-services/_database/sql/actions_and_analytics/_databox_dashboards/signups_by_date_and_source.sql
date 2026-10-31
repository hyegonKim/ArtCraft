-- User sign up histogram by method
-- Since we don't have an index on created_at, we threshold by id
-- Current ID count = 7578799 (September 28, 2024)
select
    date(created_at) as date,
    case
      when maybe_source is null then 'unknown'
      else maybe_source
    end as source,
    count(*) as signup_count
from users
where id > 7200000
and created_at >= NOW() - INTERVAL 60 DAY
group by date(created_at), maybe_source
