-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

select
    mu.id,
    mu.token,
    u.username,
    mu.media_source,
    maybe_original_filename as filename,
    original_file_size_bytes as size_bytes,
    original_duration_millis as duration_millis,
    maybe_original_mime_type as mime_type
from media_uploads as mu
    left join users AS u
        on u.token = mu.maybe_creator_user_token
order by id desc limit 10;

-- Analyze all media
select
    mu.id,
    mu.token,
    u.username,
    mu.media_source,
    maybe_original_filename as filename,
    original_file_size_bytes as size_bytes,
    original_duration_millis as duration_millis,
    maybe_original_mime_type as mime_type
from media_uploads as mu
         left join users AS u
                   on u.token = mu.maybe_creator_user_token
order by id desc
    limit 40;

-- Analyze broken media (no duration)
select
    mu.id,
    mu.token,
    u.username,
    mu.media_source,
    maybe_original_filename as filename,
    original_file_size_bytes as size_bytes,
    original_duration_millis as duration_millis,
    maybe_original_mime_type as mime_type
from media_uploads as mu
    left join users AS u
        on u.token = mu.maybe_creator_user_token
where mu.original_duration_millis = 0
order by id desc
limit 40;
