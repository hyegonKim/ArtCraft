-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Test table for write contention
-- NB: Nevermind, there's no index on `created_at`
-- select count(*) from generic_inference_jobs where created_at > now() - interval 1 minute;

-- Bin histogram by (creator user, IP address) (including logged out)
-- NB: no index on ip, hence subquery
SELECT
    maybe_creator_user_token,
    users.username as maybe_creator_username,
    creator_ip_address,
    count(*) as total_job_count
FROM (
    SELECT
        maybe_creator_user_token,
        creator_ip_address
    FROM (
        SELECT maybe_creator_user_token,
            creator_ip_address,
            created_at
        FROM generic_inference_jobs
        where status = 'pending'
        ORDER BY id DESC
        LIMIT 50000
    ) as j
    WHERE j.created_at > NOW() - INTERVAL 30 MINUTE
) as jobs
LEFT OUTER JOIN users
ON users.token = jobs.maybe_creator_user_token
group by maybe_creator_user_token, maybe_creator_username, creator_ip_address
order by total_job_count desc;


-- Bin histogram by creator user (including logged out)
-- NB: no index on ip, hence subquery
SELECT
    users.username as maybe_creator_username,
    maybe_creator_user_token,
    count(*) as total_job_count
FROM (
    SELECT
        maybe_creator_user_token,
        creator_ip_address
    FROM (
        SELECT maybe_creator_user_token,
            creator_ip_address,
            created_at
        FROM generic_inference_jobs
        ORDER BY id DESC
        LIMIT 10000
    ) as j
    WHERE j.created_at > NOW() - INTERVAL 30 MINUTE
) as jobs
LEFT OUTER JOIN users
ON users.token = jobs.maybe_creator_user_token
group by maybe_creator_user_token, maybe_creator_username
order by total_job_count desc;


-- Bin histogram by IP (logged in and non-logged in)
-- NB: no index on ip, hence subquery
SELECT
    creator_ip_address,
    count(*) as attempts
FROM (
     SELECT
         maybe_creator_user_token,
         creator_ip_address
     FROM (
          SELECT maybe_creator_user_token,
                 creator_ip_address,
                 created_at
          FROM generic_inference_jobs
          ORDER BY id DESC
              LIMIT 10000
      ) as j
     WHERE j.created_at > NOW() - INTERVAL 30 MINUTE
 ) as jobs
group by creator_ip_address
order by attempts desc;

-- Bin histogram by non-logged in IP
-- NB: no index on ip, hence subquery
SELECT
    creator_ip_address,
    count(*) as attempts
FROM (
     SELECT
         maybe_creator_user_token,
         creator_ip_address
     FROM (
          SELECT maybe_creator_user_token,
                 creator_ip_address,
                 created_at
          FROM generic_inference_jobs
          ORDER BY id DESC
              LIMIT 10000
      ) as j
     WHERE j.created_at > NOW() - INTERVAL 30 MINUTE
     AND j.maybe_creator_user_token IS NULL
 ) as jobs
group by creator_ip_address
order by attempts desc;

-- Exact IP
-- NB: no index on ip, hence subquery
SELECT
    creator_ip_address,
    count(*) as attempts
FROM (
         SELECT
             maybe_creator_user_token,
             creator_ip_address
         FROM (
                  SELECT maybe_creator_user_token,
                         creator_ip_address,
                         created_at
                  FROM generic_inference_jobs
                  ORDER BY id DESC
                      LIMIT 10000
              ) as j
         WHERE j.created_at > NOW() - INTERVAL 30 MINUTE
           AND creator_ip_address = '34.199.224.8'
     ) as jobs
group by creator_ip_address
order by attempts desc;
