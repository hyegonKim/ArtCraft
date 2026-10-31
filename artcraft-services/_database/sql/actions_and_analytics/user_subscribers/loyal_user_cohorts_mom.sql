-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Example query for date ranges:
select count(*)
from users
where created_at BETWEEN '2023-07-01 00:00:00' AND '2023-07-30 00:00:00';

--

-- Month by month subscriber usages.
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
           AND j.created_at BETWEEN '2022-12-01 00:00:00' AND '2023-01-01 00:00:00'
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
           AND j.created_at BETWEEN '2022-12-01 00:00:00' AND '2023-01-01 00:00:00'
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
           AND j.created_at BETWEEN '2022-12-01 00:00:00' AND '2023-01-01 00:00:00'
     ) as t
GROUP BY username
ORDER BY total_use_count desc;


