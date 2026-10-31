-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- NB: no date columns have an index :(
-- last id 30811607
-- id 29011606  2024-04-16 23:49:42
-- 28711606
select count(*)
from generic_inference_jobs
where id > 28711606
--where created_at > (CURDATE() - INTERVAL 1 MINUTE)

select avg(success_execution_millis), inference_category
from generic_inference_jobs
where id > 28711606
and maybe_creator_user_token IS NULL
and status = 'complete_success'
group by inference_category


select avg(success_execution_millis), inference_category, count(*)
from generic_inference_jobs
where id > 28711606
  and maybe_creator_user_token IS NULL
  and status = 'complete_success'
group by inference_category

-- TODO: generations per user
select inference_category, count(*)
from (
    select maybe_creator_anonymous_visitor_token, inference_category, count(*)
    from generic_inference_jobs
    where id > 28711606
    and maybe_creator_user_token IS NULL
    group by inference_category, maybe_creator_anonymous_visitor_token
) as sub
group by sub.inference_category



select avg(success_execution_millis), inference_category, count(*), subscription_product_slug
from generic_inference_jobs
join users
on users.token = generic_inference_jobs.maybe_creator_user_token
left outer join user_subscriptions
on user_subscriptions.user_token = users.token
where generic_inference_jobs.id > 28711606
  and status = 'complete_success'
group by inference_category, user_subscriptions.subscription_product_slug

-- Number of unique users for each

-- Tammie ask:  "# users who generated anything by payment tier, not generation type"

-- Number of generations by payment tier
select subscription_product_slug, count(*)
from generic_inference_jobs
left outer join users
   on users.token = generic_inference_jobs.maybe_creator_user_token
left outer join user_subscriptions
   on user_subscriptions.user_token = users.token
where generic_inference_jobs.created_at > (CURDATE() - INTERVAL 14 DAY)
  and status = 'complete_success'
group by subscription_product_slug

-- and generic_inference_jobs.id > 28711606


-- Count of unique paying and non-paying users ("null") that have used
-- at least one product at least once in the last 14 days
SELECT subscription_product_slug, count(*)
FROM
(
  select
    distinct maybe_creator_user_token as user_token
  from generic_inference_jobs
    where maybe_creator_user_token IS NOT NULL
    and generic_inference_jobs.created_at > (CURDATE() - INTERVAL 14 DAY)
) as u
LEFT OUTER JOIN user_subscriptions
on user_subscriptions.user_token = u.user_token
group by subscription_product_slug;

-- Unique anonymous users who have used at least one product in the last 14 days
select count(distinct maybe_creator_anonymous_visitor_token) as anonymous_users
from generic_inference_jobs
where maybe_creator_user_token IS NULL
  and generic_inference_jobs.created_at > (CURDATE() - INTERVAL 14 DAY);
