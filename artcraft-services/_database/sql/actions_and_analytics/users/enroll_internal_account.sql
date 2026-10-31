-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Enroll internal accounts in studio + loyalty features
UPDATE users
SET
    can_access_studio = true,
    maybe_feature_flags = 'explore_media,studio,video_style_transfer',
    maybe_loyalty_program_key = 'fakeyou_contributor',
    user_role_slug = "admin"
WHERE username IN (
  '48hourfilm',
  'intro_video',
  'zzz_last_item'
)
LIMIT 5;
