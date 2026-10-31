-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile


-- Inventory non-deleted
select
    m.id,
    m.token,
    SUBSTRING(m.title, 1, 35) as title,
    u.username,
    SUBSTRING(m.original_download_url, 1, 35) as url,
    m.updated_at
from model_weights as m
    left join users AS u on
        u.token = m.creator_user_token
where
    m.user_deleted_at IS NULL
  and m.mod_deleted_at IS NULL
order by m.title asc;

-- Top models within time period
-- NB: id threshold is to avoid a full table scan
select
    mw.token,
    mw.title,
    mw.weights_type,
    mw.weights_category,
    u.username as creator_username,
    r.use_count,
    mw.created_at,
    mw.user_deleted_at,
    mw.mod_deleted_at
from (
    select maybe_model_token, count(*) as use_count
    from generic_inference_jobs
    where maybe_model_token IS NOT NULL
        and created_at > NOW() - INTERVAL 7 DAY
        and id > 40000000
    group by maybe_model_token
) as r
join model_weights as mw
    on mw.token = r.maybe_model_token
join users as u
    on u.token = mw.creator_user_token
order by r.use_count desc
    limit 100;
