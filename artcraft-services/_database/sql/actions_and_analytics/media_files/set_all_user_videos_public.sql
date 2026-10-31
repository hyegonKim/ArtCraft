-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Set all user videos public
-- NB: using join since subquery was doing a full table scan despite looking reasonable.
UPDATE media_files m INNER JOIN users
ON m.maybe_creator_user_token = users.token
AND m.media_class  = 'video'
  SET m.creator_set_visibility = 'public'
WHERE m.creator_set_visibility IN ('private', 'hidden')
AND users.username IN (
  'devdude123',
  'dreambig',
  'gateway',
  'heart_ribbon',
  'postproduction',
  'tellstories',
  'yae_ph'
)
