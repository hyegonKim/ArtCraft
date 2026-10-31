-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- All logged in users with results in the last X days
select u.created_at from users as u
where u.token IN
(select distinct u.token from tts_inference_jobs as j join users u on u.token = j.maybe_creator_user_token where j.created_at > now() - interval 7 day)
order by u.created_at ASC;


-- All logged in users with results in the last X days that are older than Y
select u.created_at from users as u
where u.token IN
(select distinct u.token from tts_inference_jobs as j join users u on u.token = j.maybe_creator_user_token where j.created_at > now() - interval 30 day)
AND u.created_at < now() - interval 30 day
order by u.created_at ASC;

-- ====================== FEBRUARY COHORT ===================================

-- [1] All users created between X and Y
select count(*) from users
where created_at > now() - interval 60 day
and created_at < now() - interval 30 day;

-- [2] Active users from that cohort ^ 
select count(*) from users as u
where u.token IN
(select distinct u.token from tts_inference_jobs as j join users u on u.token = j.maybe_creator_user_token where j.created_at > now() - interval 30 day)
AND created_at > now() - interval 60 day
AND created_at < now() - interval 30 day;



-- ====================== JANUARY COHORT ===================================

-- [1] All users created between X and Y
select count(*) from users
where created_at > now() - interval 90 day
and created_at < now() - interval 60 day;

-- [2] Active users from that cohort ^ 
select count(*) from users as u
where u.token IN
(select distinct u.token from tts_inference_jobs as j join users u on u.token = j.maybe_creator_user_token where j.created_at > now() - interval 30 day)
AND created_at > now() - interval 90 day
AND created_at < now() - interval 60 day;



(select distinct u.token from tts_inference_jobs as j join users u on u.token = j.maybe_creator_user_token where j.created_at > now() - interval 30 day)


select j.creator_ip_address, count(*) as number 
from tts_inference_jobs as j 
where j.created_at > now() - interval 1 day 
group by j.creator_ip_address 
order by number desc 
limit 100;


select count(*) from tts_inference_jobs 
where created_at > now() - interval 1 day ;

select count(distinct creator_ip_address) from tts_inference_jobs 
where created_at > now() - interval 1 day ;



select j.creator_ip_address, count(*) as number 
from tts_inference_jobs as j 
where j.created_at > now() - interval 1 day 
group by j.creator_ip_address 
order by number desc 
limit 100;



