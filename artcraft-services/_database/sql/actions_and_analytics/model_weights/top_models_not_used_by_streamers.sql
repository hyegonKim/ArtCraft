
-- Select popular models without streamer usage distorting the totals
select
    mw.token,
    mw.title,
    count(*) as use_count
from generic_inference_jobs as gij
left outer join model_weights as mw
    on gij.maybe_model_token = mw.token
where gij.maybe_creator_user_token NOT IN (
    -- This subquery returns streamers that generated over a certain threshold number of results
    select
        user_token
    from (
        select
            u.token as user_token,
            count(*) as generations
        from generic_inference_jobs as gij
        join users as u
            on gij.maybe_creator_user_token = u.token
        where gij.created_at >= (CURDATE() - INTERVAL 5 DAY)
        and gij.maybe_creator_user_token is not null
        group by user_token
        order by generations desc
    ) as top_users
    where top_users.generations > 1000
)
and gij.created_at >= (CURDATE() - INTERVAL 5 DAY)
group by mw.token, mw.title
order by use_count desc
limit 100;


