SELECT
  m.media_type,
  COUNT(*)
FROM media_files as m
JOIN users as u
  ON m.maybe_creator_user_token = u.token
WHERE u.username = lower('dzth')
GROUP BY m.media_type


