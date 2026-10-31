-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile


-- Number of paid users (inaccurate)
select count(*) from users where maybe_stripe_customer_id IS NOT NULL;

-- Number of paid users (inaccurate)
select count(*) from user_subscriptions;

-- Number of paid users
-- NB: "active" status users are current, treating "past_due" as active too (for now)
select count(*) from user_subscriptions where maybe_stripe_subscription_status NOT IN ("canceled", "incomplete", "incomplete_expired");

-- ==================== TTS + VC + W2L ====================

-- Make sure the time intervals are the same!
SELECT
    username,
    count(*) as total_use_count
FROM (
   SELECT
        u.username as username,
        j.token as token
    FROM users AS u
    JOIN tts_inference_jobs AS j
        ON u.token = j.maybe_creator_user_token
        WHERE u.token IN (
            SELECT DISTINCT user_token
            FROM user_subscriptions
        )
        AND j.created_at > NOW() - INTERVAL 31 DAY
   UNION
        SELECT
            u.username as username,
            j.token as token
        FROM users AS u
            JOIN generic_inference_jobs AS j
        ON u.token = j.maybe_creator_user_token
        WHERE u.token IN (
            SELECT DISTINCT user_token
            FROM user_subscriptions
            )
          AND j.created_at > NOW() - INTERVAL 31 DAY
    UNION
        SELECT
            u.username as username,
            j.token as token
        FROM users AS u
             JOIN w2l_inference_jobs AS j
        ON u.token = j.maybe_creator_user_token
        WHERE u.token IN (
            SELECT DISTINCT user_token
            FROM user_subscriptions
            )
        AND j.created_at > NOW() - INTERVAL 31 DAY
) as t
GROUP BY username
ORDER BY total_use_count desc

-- ==================== TEXT TO SPEECH====================

-- All paid subscribers generating results in the last week
select u.username, count(*) as use_count
from users as u
join tts_inference_jobs as j
on u.token = j.maybe_creator_user_token
WHERE u.token IN (
    select distinct user_token
    from user_subscriptions
    -- where maybe_stripe_subscription_status NOT IN ("canceled", "incomplete", "incomplete_expired")
)
AND j.created_at > NOW() - interval 7 day
GROUP BY u.username
ORDER BY use_count desc

-- Paid subscribers generating results in the last month (with # threshold)
select username, use_count
FROM (
    select u.username, count(*) as use_count
    from users as u
             join tts_inference_jobs as j
                  on u.token = j.maybe_creator_user_token
    WHERE u.token IN (
        select distinct user_token
        from user_subscriptions
        where maybe_stripe_subscription_status NOT IN ("canceled", "incomplete", "incomplete_expired")
    )
    AND j.created_at > NOW() - interval 31 day
    GROUP BY u.username
    ORDER BY use_count desc
) as t
WHERE use_count > 10

-- Paid users generating results in the last month
select u.username, count(*) as use_count
from users as u
      join tts_inference_jobs as j
           on u.token = j.maybe_creator_user_token
WHERE u.token IN (
     select distinct user_token
     from user_subscriptions
     where maybe_stripe_subscription_status NOT IN ("canceled", "incomplete", "incomplete_expired")
)
AND j.created_at > NOW() - interval 1 day
GROUP BY u.username
ORDER BY use_count desc

-- ==================== WAV2LIP ====================

select u.username, count(*) as use_count
from users as u
         join w2l_inference_jobs as j
              on u.token = j.maybe_creator_user_token
WHERE u.token IN (
    select distinct user_token
    from user_subscriptions
    where maybe_stripe_subscription_status NOT IN ("canceled", "incomplete", "incomplete_expired")
)
  AND j.created_at > NOW() - interval 31 day
GROUP BY u.username
ORDER BY use_count desc

select username, use_count
FROM (
         select u.username, count(*) as use_count
         from users as u
                  join w2l_inference_jobs as j
                       on u.token = j.maybe_creator_user_token
         WHERE u.token IN (
             select distinct user_token
             from user_subscriptions
             where maybe_stripe_subscription_status NOT IN ("canceled", "incomplete", "incomplete_expired")
         )
           AND j.created_at > NOW() - interval 31 day
         GROUP BY u.username
         ORDER BY use_count desc
     ) as t
WHERE use_count >=5;

-- ==================== TTS MODELS ====================

select u.username, count(*) as use_count
from users as u
         join tts_model_upload_jobs as j
              on u.token = j.creator_user_token
WHERE u.token IN (
    select distinct user_token
    from user_subscriptions
    where maybe_stripe_subscription_status NOT IN ("canceled", "incomplete", "incomplete_expired")
)
  AND j.created_at > NOW() - interval 1 day
GROUP BY u.username
ORDER BY use_count desc

-- ==================== W2L TEMPLATES ====================

select u.username, count(*) as use_count
from users as u
         join w2l_template_upload_jobs as j
              on u.token = j.creator_user_token
WHERE u.token IN (
    select distinct user_token
    from user_subscriptions
    where maybe_stripe_subscription_status NOT IN ("canceled", "incomplete", "incomplete_expired")
)
  AND j.created_at > NOW() - interval 1 day
GROUP BY u.username
ORDER BY use_count desc
