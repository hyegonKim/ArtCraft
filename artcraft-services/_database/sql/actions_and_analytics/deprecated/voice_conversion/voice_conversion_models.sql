-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Inventory all
select
    vc.id,
    vc.token,
    vc.model_type,
    vc.title,
    u.username,
    vc.original_download_url,
    vc.created_at,
    vc.updated_at,
    vc.user_deleted_at,
    vc.mod_deleted_at,
    vc.maybe_mod_comments
from voice_conversion_models as vc
left join users AS u on
    u.token = vc.creator_user_token;

-- Inventory non-deleted
select
    vc.id,
    vc.token,
    vc.model_type,
    SUBSTRING(vc.title, 1, 35) as title,
    u.username,
    SUBSTRING(vc.original_download_url, 1, 35) as url,
    vc.updated_at
from voice_conversion_models as vc
left join users AS u on
    u.token = vc.creator_user_token
where
    vc.user_deleted_at IS NULL
    and vc.mod_deleted_at IS NULL
order by vc.title asc;

-- Inventory duplicate non-deleted models
select
    vc.id,
    vc.token,
    vc.model_type,
    vc.title,
    u.username,
    vc.original_download_url,
    vc.created_at,
    vc.updated_at
from voice_conversion_models as vc
         left join users AS u on
            u.token = vc.creator_user_token
where vc.original_download_url IN (
    select original_download_url from (
      select original_download_url,
             count(*) as duplicate_count
      from voice_conversion_models
      where user_deleted_at IS NULL
        and mod_deleted_at IS NULL
      group by original_download_url
      order by original_download_url desc
    ) as duplicates
    where duplicate_count > 1
)
and vc.user_deleted_at IS NULL
and vc.mod_deleted_at IS NULL
order by
    vc.original_download_url asc,
    id asc;

-- order by vc.original_download_url ASC;

--
-- Top 100 voice conversion models by use count
-- (jobs table, not results table)
--
select
    m.token,
    m.model_type,
    m.title,
    u.username,
    r.use_count,
    m.created_at,
    m.user_deleted_at,
    m.mod_deleted_at
from (
         select maybe_model_token, count(*) as use_count
         from generic_inference_jobs
         where maybe_model_token IS NOT NULL
         group by maybe_model_token
     ) as r
         join voice_conversion_models as m
              on m.token = r.maybe_model_token
         join users as u
              on u.token = m.creator_user_token
order by r.use_count desc
    limit 100;


--
-- Top 100 voice conversion models by use count (NON DELETED)
-- (jobs table, not results table)
--
select
    m.token,
    m.model_type,
    m.title,
    u.username,
    r.use_count,
    m.created_at
from (
     select maybe_model_token, count(*) as use_count
     from generic_inference_jobs
     where maybe_model_token IS NOT NULL
     group by maybe_model_token
) as r
join voice_conversion_models as m
    on m.token = r.maybe_model_token
join users as u
    on u.token = m.creator_user_token
where m.user_deleted_at IS NULL
and m.mod_deleted_at IS NULL
order by r.use_count desc
limit 100;


--
-- Top 100 voice conversion models by use count (NON DELETED, LAST 7 DAYS)
-- (jobs table, not results table)
--
select
    m.token,
    m.model_type,
    m.title,
    u.username,
    r.use_count,
    m.created_at
from (
    select maybe_model_token, count(*) as use_count
    from generic_inference_jobs
    where maybe_model_token IS NOT NULL
    and created_at > ( CURDATE() - INTERVAL 7 day)
    group by maybe_model_token
) as r
join voice_conversion_models as m
    on m.token = r.maybe_model_token
join users as u
    on u.token = m.creator_user_token
where m.user_deleted_at IS NULL
  and m.mod_deleted_at IS NULL
order by r.use_count desc
    limit 100;



-- Histogram of model contributions
select
    date(created_at) as created_date,
    count(*) as model_count
from voice_conversion_models
group by created_date

-- Histogram of model contributions (within range)
select
    date(created_at) as created_date,
    count(*) as model_count
from voice_conversion_models
where created_at > (CURDATE() - INTERVAL 5 DAY)
group by created_date

