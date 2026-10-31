
select
  u.username,
  count(*) as generations
from generic_inference_jobs as gij
join users as u
  on gij.maybe_creator_user_token = u.token
where gij.created_at >= (CURDATE() - INTERVAL 5 DAY)
and gij.maybe_creator_user_token is not null
group by u.username
order by generations desc
limit 10