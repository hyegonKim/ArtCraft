-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

UPDATE media_files
SET is_user_upload = TRUE
WHERE origin_category IN ('upload', 'device_api')
AND is_user_upload = FALSE
LIMIT 10000;

SELECT COUNT(*)
FROM media_files
WHERE origin_category IN ('upload', 'device_api')
AND is_user_upload = FALSE;