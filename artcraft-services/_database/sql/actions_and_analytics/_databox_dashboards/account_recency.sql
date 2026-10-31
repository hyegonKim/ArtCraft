-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Generations by account recency histogram
select
    created_date,
    case when user_account_age_days <= 1 then 'today'
    when user_account_age_days <= 30 then '<1 month'
    when user_account_age_days <= 60 then '<2 month'
    when user_account_age_days <= 90 then '<3 month'
    when user_account_age_days <= 365 then '<1 year'
    else '1+ years' end as user_account_age,
    count(*) as generations
FROM (
    SELECT
        DATE(gij.created_at) as created_date,
        -- Account age at time of media generation
        DATEDIFF(gij.created_at, u.created_at) as user_account_age_days
    from
        generic_inference_jobs as gij
    join
        users as u
    on
        gij.maybe_creator_user_token = u.token
    where
        gij.created_at >= (CURDATE() - INTERVAL 30 DAY)
        and gij.status = 'complete_success'
        and gij.id > 40839206
    order by gij.created_at asc
) as x
group by created_date, user_account_age
order by created_date asc, user_account_age asc;

