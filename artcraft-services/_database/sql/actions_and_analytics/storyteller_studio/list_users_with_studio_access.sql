-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Find users with studio flags.
select username from users where maybe_feature_flags  like '%studio%';

select username from users where can_access_studio = true;

-- Who signed up recently and has access?
select username, maybe_source
from users where can_access_studio = true
and created_at > NOW() - interval 20 minute;
