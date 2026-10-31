-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

--
-- Usage count in the past 24 hours
--
select count(*) as use_count
from tts_results
where created_at > ( CURDATE() - INTERVAL 1 DAY )

--
-- Usage count in the past week
--
select count(*) as use_count
from tts_results
where created_at > ( CURDATE() - INTERVAL 7 DAY )

-- On-prem worker mix
select count(*) as use_count
from tts_results
where created_at > ( CURDATE() - INTERVAL 1 MINUTE )
and is_generated_on_prem IS TRUE

-- Calculate a percentage of on-prem worker capacity
select count(*) as on_prem_count
from (
  select is_generated_on_prem
  from tts_results
  order by id desc limit 1000
) as sample
where sample.is_generated_on_prem IS TRUE;

-- Histogram of days of content per day
select
    date(created_at) as created_date,
    sum(duration_millis) / 1000 / 60 / 60 / 24 as days
from tts_results
group by created_date

-- Histogram of days of content per day (range)
select
    date(created_at) as created_date,
    sum(duration_millis) / 1000 / 60 / 60 / 24 as days
from tts_results
where date(created_at) >= "2023-01-01"
group by created_date

-- Histogram of days of content per day (range)
-- Maybe non-tablescan?
select
    date(created_at) as created_date,
    sum(duration_millis) / 1000 / 60 / 60 / 24 as days
from tts_results
where created_at > (CURDATE() - INTERVAL 30 DAY)
group by created_date

-- Find TTS results for a single model
select count(*) from tts_results
where model_token = 'TM:ztt5s1be5tq6';

-- Find TTS results for a single model that are not deleted
select count(*) from tts_results
where model_token = 'TM:ztt5s1be5tq6'
and mod_deleted_at is NULL;

-- Sample results for a single model
select
    TRIM(REPLACE(SUBSTRING(raw_inference_text, 1, 50), '\n', ''))
from tts_results
where model_token='TM:yt4gfbkngsjj'
order by id desc
limit 150;

-- Search for results by eminem
select
    token,
    created_at,
    raw_inference_text
from  tts_results
where
    model_token IN ('TM:27fj0gsh11pd', 'TM:8h9bfjgabeer', 'TM:pdf9c1anbdjq')
    AND raw_inference_text LIKE "%rave%"
  AND raw_inference_text LIKE "%sound%"
  AND created_at > ( CURDATE() - INTERVAL 5 MONTH )
ORDER BY created_at DESC



--- Search for particular text by a particular model
SELECT
    raw_inference_text,
    maybe_creator_user_token,
    created_at
FROM
    tts_results
WHERE
    model_token = 'TM:ztt5s1be5tq6'
    AND raw_inference_text LIKE '%fakeyou%'

--- Search for 101soundboards.com using us
SELECT
    r.raw_inference_text,
    r.maybe_creator_user_token,
    u.username,
    u.ip_address_creation,
    r.creator_ip_address,
    r.created_at
FROM
    tts_results as r
LEFT OUTER JOIN
    users as u
ON
    r.maybe_creator_user_token = u.token
WHERE
    r.raw_inference_text LIKE '%bananas%'
    AND r.created_at > ( CURDATE() - INTERVAL 1 HOUR )


--- Search for 101soundboards.com using us
SELECT
    r.maybe_creator_user_token, r.creator_ip_address, r.raw_inference_text
FROM
    tts_results as r
        LEFT OUTER JOIN
    users as u
    ON
            r.maybe_creator_user_token = u.token
WHERE
        r.raw_inference_text LIKE '%pickle%'
  AND r.created_at > ( CURDATE() - INTERVAL 1 HOUR )


---
-- WTF WHY IS THIS SLOW???
SELECT
    res.id,
    model.title,
    res.raw_inference_text,
    res.maybe_creator_user_token
FROM
    tts_results AS res
JOIN
    tts_models as model
ON
    res.model_token = model.token
ORDER BY res.id
LIMIT 10;

