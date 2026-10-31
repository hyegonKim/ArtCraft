-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

--
-- Top 100 models all time by use count
--
select
    m.token,
    m.text_pipeline_type,
    m.ietf_language_tag,
    m.title,
    u.username,
    r.use_count,
    m.created_at,
    m.user_deleted_at,
    m.mod_deleted_at
from (
         select model_token, count(*) as use_count
         from tts_results
         group by model_token
     ) as r
         join tts_models as m
              on m.token = r.model_token
         join users as u
              on u.token = m.creator_user_token
order by r.use_count desc
    limit 100;

--
-- Same, but simpler...
--
select m.token, m.title, r.use_count from (
  select model_token, count(*) as use_count from tts_results
  group by model_token
  order by use_count desc limit 100
) as r
  join tts_models as m
  on m.token = r.model_token;

--
-- Top 100 models by use count, last 5 days
-- Limited due to tmux scrollback.
--
select
    m.token,
    m.text_pipeline_type,
    m.ietf_language_tag,
    m.title,
    u.username,
    r.use_count,
    m.created_at,
    m.user_deleted_at,
    m.mod_deleted_at
from (
    select model_token, count(*) as use_count
    from tts_results
    where created_at > ( CURDATE() - INTERVAL 12 HOUR )
    group by model_token
) as r
    join tts_models as m
    on m.token = r.model_token
    join users as u
    on u.token = m.creator_user_token
order by r.use_count desc
    limit 100;


--
-- Top models binned by IP address.
-- Should give us a look at demand independent of model quality / repeated use.
--
select
    m.token,
    m.title,
    m.user_ratings_positive_count / m.user_ratings_total_count as rating,
    m.text_pipeline_type,
    m.ietf_language_tag,
    u.username,
    r.use_count,
    m.created_at,
    m.user_deleted_at,
    m.mod_deleted_at
from (
        select model_token, count(*) as use_count
        from (
            select model_token, creator_ip_address
            from tts_results
            where created_at > (CURDATE() - INTERVAL 12 HOUR)
            group by model_token, creator_ip_address
        ) as x
        group by model_token
     ) as r
         join tts_models as m
              on m.token = r.model_token
         join users as u
              on u.token = m.creator_user_token
         where m.ietf_language_tag IN ('en', 'en-US', 'en-AU', 'en-CA', 'en-GB')
order by r.use_count desc
    limit 100;

--
-- Top 20 ***NEW*** models to share in #announcements
-- Can be scoped by creator.
--
select
    m.token,
    m.text_pipeline_type,
    m.ietf_language_tag,
    m.creator_set_visibility,
    m.title,
    u.username,
    r.use_count,
    m.created_at,
    m.user_deleted_at,
    m.mod_deleted_at
from (
         select model_token, count(*) as use_count
         from tts_results
         where created_at > ( CURDATE() - INTERVAL 5 DAY )
         group by model_token
     ) as r
         join tts_models as m
              on m.token = r.model_token
         join users as u
              on u.token = m.creator_user_token
where m.created_at > ( CURDATE() - INTERVAL 31 DAY )
  -- AND u.username IN ( 'vegito1089', 'justinjohn0306')
order by r.use_count desc
    limit 100;

--
-- Top 100 models by use count, last 5 days, not spanish, etc.
-- Limited due to tmux scrollback.
--
select
    m.token,
    m.text_pipeline_type,
    m.ietf_language_tag,
    m.title,
    u.username,
    r.use_count,
    m.created_at,
    m.user_deleted_at,
    m.mod_deleted_at
from (
    select model_token, count(*) as use_count
    from tts_results
    where created_at > ( CURDATE() - INTERVAL 5 DAY )
    group by model_token
) as r
    join tts_models as m
    on m.token = r.model_token
    join users as u
    on u.token = m.creator_user_token
    where m.ietf_language_tag NOT IN ('es', 'es-419', 'es-ES', 'es-MX', 'pt-BR')
order by r.use_count desc
    limit 100;

--
-- Top 100 models by use count, last 5 days, in specific language.
-- Limited due to tmux scrollback.
--
-- English: ('en', 'en-US', 'en-AU', 'en-CA', 'en-GB')
-- Spanish: ('es', 'es-419', 'es-AR', 'es-CL', 'es-CO', 'es-ES', 'es-MX', 'es-PE', 'es-US')
-- Italian: ('it', 'it-CH', 'it-IT')
-- Portuguese: ('pt', 'pt-BR')
--
select
    m.token,
    m.title,
    m.text_pipeline_type,
    m.ietf_language_tag,
    u.username,
    r.use_count,
    m.created_at,
    m.user_deleted_at,
    m.mod_deleted_at
from (
         select model_token, count(*) as use_count
         from tts_results
         where created_at > ( CURDATE() - INTERVAL 30 DAY )
         group by model_token
     ) as r
         join tts_models as m
              on m.token = r.model_token
         join users as u
              on u.token = m.creator_user_token
where m.ietf_language_tag IN ('en', 'en-US', 'en-AU', 'en-CA', 'en-GB')
order by r.use_count desc
    limit 100;

--
-- Most popular voices by use count over 5-day window, single language, single user.
--
select
    m.token,
    m.title,
    m.ietf_language_tag,
    u.username,
    r.use_count,
    m.created_at,
    m.user_deleted_at,
    m.mod_deleted_at
from (
    select model_token, count(*) as use_count
    from tts_results
    where created_at > ( CURDATE() - INTERVAL 5 DAY )
    group by model_token
) as r
    join tts_models as m
    on m.token = r.model_token
    join users as u
    on u.token = m.creator_user_token
    where
        m.ietf_language_tag NOT IN ('es', 'es-419', 'es-ES', 'es-MX', 'pt-BR')
        AND u.username = 'vegito1089'
order by r.use_count desc
    limit 100;

--
-- Most popular deleted models
--
select
    m.token,
    m.title,
    m.ietf_language_tag,
    u.username,
    r.use_count,
    m.created_at,
    m.user_deleted_at,
    m.mod_deleted_at
from (
    select model_token, count(*) as use_count
    from tts_results
    where model_token IN (
        select token
        from tts_models
        where
            user_deleted_at IS NOT NULL
            OR mod_deleted_at IS NOT NULL
    )
    group by model_token
) as r
    join tts_models as m
    on m.token = r.model_token
    join users as u
    on u.token = m.creator_user_token
order by r.use_count desc
    limit 100;

--
-- Most popular deleted models (only by users, not mods)
-- Mod-deleted models were probably on purpose
--
select
    m.token,
    m.title,
    m.ietf_language_tag,
    u.username,
    r.use_count,
    m.created_at,
    m.user_deleted_at
from (
         select model_token, count(*) as use_count
         from tts_results
         where model_token IN (
             select token
             from tts_models
             where
                 user_deleted_at IS NOT NULL
         )
         group by model_token
     ) as r
         join tts_models as m
              on m.token = r.model_token
         join users as u
              on u.token = m.creator_user_token
order by r.use_count desc
    limit 100;


--
-- Models uploaded recently (to check if they use the right text_pipeline_type,
-- vocoder -todo-, etc.)
--
select
    m.token,
    m.title,
    m.text_pipeline_type,
    m.ietf_language_tag,
    u.username,
    r.use_count,
    m.created_at,
    m.user_deleted_at,
    m.mod_deleted_at
from (
         select model_token, count(*) as use_count
         from tts_results
         where created_at > ( CURDATE() - INTERVAL 5 DAY )
         group by model_token
     ) as r
         join tts_models as m
              on m.token = r.model_token
         join users as u
              on u.token = m.creator_user_token
order by m.created_at desc
    limit 100;

-- Histogram of model contributions
select
    date(created_at) as created_date,
    count(*) as model_count
from tts_models
group by created_date

-- Histogram of model contributions (within range)
select
    date(created_at) as created_date,
    count(*) as model_count
from tts_models
where created_at > (CURDATE() - INTERVAL 5 DAY)
group by created_date

--
-- Models deleted recently (voice actor take down)
--
select
    m.token,
    m.title,
    m.text_pipeline_type,
    m.ietf_language_tag,
    u.username,
    r.use_count,
    m.created_at,
    m.user_deleted_at,
    m.mod_deleted_at
from (
         select model_token, count(*) as use_count
         from tts_results
         where created_at > ( CURDATE() - INTERVAL 30 DAY )
         group by model_token
     ) as r
         join tts_models as m
              on m.token = r.model_token
         join users as u
              on u.token = m.creator_user_token
where m.mod_deleted_at > ( CURDATE() - INTERVAL 14 DAY )
   or m.user_deleted_at > ( CURDATE() - INTERVAL 14 DAY )
order by r.use_count desc
    limit 500;

--
-- Non-public models
--
select
    m.token,
    m.title,
    m.text_pipeline_type,
    m.ietf_language_tag,
    m.creator_set_visibility,
    u.username,
    r.use_count,
    m.created_at,
    m.user_deleted_at,
    m.mod_deleted_at
from (
         select model_token, count(*) as use_count
         from tts_results
         where created_at > ( CURDATE() - INTERVAL 30 DAY )
         group by model_token
     ) as r
         left outer join tts_models as m
              on m.token = r.model_token
         join users as u
              on u.token = m.creator_user_token
where m.creator_set_visibility != 'public'
order by r.use_count desc
    limit 500;

