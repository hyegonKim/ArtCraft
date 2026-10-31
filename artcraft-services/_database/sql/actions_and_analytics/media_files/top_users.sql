
-- Top users in a time period
select
  u.username,
  u.token,
  u.created_at,
  count(*) as generations
from
  media_files as m
  join users as u
    on m.maybe_creator_user_token = u.token
where
  m.created_at >= (CURDATE() - INTERVAL 5 DAY)
group by
  u.username, u.token, u.created_at
order by
  generations desc
limit 50;


-- Top users all time
select
  u.username,
  u.token,
  u.created_at,
  count(*) as generations
from
  media_files as m
  join users as u
    on m.maybe_creator_user_token = u.token
group by
  u.username, u.token, u.created_at
order by
  generations desc
limit 500;
