-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Set all featured media items public
-- NB: using join since subquery was doing a full table scan despite looking reasonable.
UPDATE media_files m INNER JOIN featured_items items
ON items.entity_token = m.token
AND items.entity_type = 'media_file'
  SET m.creator_set_visibility = 'public'
WHERE m.creator_set_visibility IN ('private', 'hidden')
AND items.deleted_at IS NULL;
