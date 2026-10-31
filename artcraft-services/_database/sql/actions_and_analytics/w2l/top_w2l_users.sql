-- Top w2l users
select
  distinct maybe_creator_user_token,
  count(*) as creations,
  u.username,
  u.email_address,
  u.maybe_stripe_customer_id,
  u.created_at
from w2l_inference_jobs w
left outer join users u
  on u.token = w.maybe_creator_user_token
group by maybe_creator_user_token
order by creations desc
