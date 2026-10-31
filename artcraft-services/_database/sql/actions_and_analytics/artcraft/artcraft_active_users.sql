-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

select
    u.display_name,
    count(*) as sessions,
    a.session_duration_seconds / 60 / 60 as hours,
    a.measurement_count,
    u.email_address,
    a.os_platform,
    a.os_version,
    u.created_at,
    u.maybe_source,
    a.last_active_at
from analytics_app_sessions as a
left outer join users as u
on a.user_token = u.token
group by a.user_token
order by hours desc;