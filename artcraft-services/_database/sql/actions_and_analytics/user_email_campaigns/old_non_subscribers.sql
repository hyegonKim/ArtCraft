

-- Older user accounts that dropped off
select
  username,
  email_address
from
  users
where token IN (
  -- Users that used us long ago
  select maybe_creator_user_token
  from tts_results
  where maybe_creator_user_token IS NOT NULL
  and created_at < NOW() - INTERVAL 23 MONTH
  and created_at > NOW() - INTERVAL 24 MONTH
  group by maybe_creator_user_token
)
and token NOT IN (
  -- Users that used us recently
  select maybe_creator_user_token
  from media_files
  where maybe_creator_user_token IS NOT NULL
  and created_at > NOW() - INTERVAL 3 MONTH
  group by maybe_creator_user_token
)
and maybe_stripe_customer_id IS NULL
