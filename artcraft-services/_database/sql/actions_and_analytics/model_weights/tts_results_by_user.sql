
-- Count the most-used tts models for a given user
-- This is a useful query to distinguish "AI Streamers" from API abusers
-- This query has old TTS model token support!
SELECT
   mw.token,
   mw.title,
   count(*) as use_count
FROM model_weights as mw
JOIN (
    select
      coalesce(w.token, w_migrated.token) as token
    from media_files as f
    left outer join model_weights as w
       on f.maybe_origin_model_token = w.token
    left outer join model_weights as w_migrated
      on f.maybe_origin_model_token = w_migrated.maybe_migration_old_model_token
    join users as u
      on f.maybe_creator_user_token = u.token
    where u.username = lower('peepostream')
) as x
on mw.token = x.token
group by mw.token, mw.title
order by use_count desc


-- See a sample of tts results by user
-- This is a useful query to distinguish "AI Streamers" from API abusers
-- This includes old TTS model token support
select
  f.token,
  f.origin_product_category,
  f.created_at,
  w.token,
  w.title,
  w_migrated.token,
  w_migrated.title,
  SUBSTRING(TRIM(f.maybe_text_transcript), 1, 100) as transcript
from media_files as f
left outer join model_weights as w
  on f.maybe_origin_model_token = w.token
left outer join model_weights as w_migrated
  on f.maybe_origin_model_token = w_migrated.maybe_migration_old_model_token
join users as u
  on f.maybe_creator_user_token = u.token
where u.username = lower('peepostream')
order by f.id desc
limit 100

