
-- This query works!
-- Here we get a histogram of model usages on a particular day.
-- This fetches counts of all jobs that were submitted using either
-- a modern model_weights token or a legacy tts_model token and sums
-- them together when the two correspond to the same model_weight
-- record.
SELECT
  distinct coalesce(w.token, w_old.token) as token,
  COUNT(*) as usage_count
FROM
    media_files as f
LEFT OUTER JOIN model_weights as w
   ON f.maybe_origin_model_token = w.token
LEFT OUTER JOIN model_weights as w_old
   ON f.maybe_origin_model_token = w_old.maybe_migration_old_model_token
WHERE f.maybe_origin_model_token IS NOT NULL
  AND f.created_at >= '2024-09-01'
  AND f.created_at < '2024-09-01' + INTERVAL 1 DAY
GROUP BY coalesce(w.token, w_old.token)
ORDER BY usage_count DESC
LIMIT 10;

-- TESTING: Above query, but only modern model_weights tokens
SELECT
  distinct w.token,
  COUNT(*) as usage_count
FROM
    media_files as f
JOIN model_weights as w
   ON f.maybe_origin_model_token = w.token
WHERE f.maybe_origin_model_token IS NOT NULL
  AND f.created_at >= '2024-09-01'
  AND f.created_at < '2024-09-01' + INTERVAL 1 DAY
GROUP BY w.token
ORDER BY usage_count DESC
LIMIT 10;

-- TESTING: Above query, but only legacy tts_models tokens
SELECT
  distinct w_old.token,
  COUNT(*) as usage_count
FROM
    media_files as f
LEFT OUTER JOIN model_weights as w_old
   ON f.maybe_origin_model_token = w_old.maybe_migration_old_model_token
WHERE f.maybe_origin_model_token IS NOT NULL
  AND f.created_at >= '2024-09-01'
  AND f.created_at < '2024-09-01' + INTERVAL 1 DAY
GROUP BY w_old.token
ORDER BY usage_count DESC
LIMIT 10;

-- TESTING: Testing that the math works without joining
SELECT
  distinct f.maybe_origin_model_token,
  COUNT(*) as usage_count
FROM
    media_files as f
WHERE f.maybe_origin_model_token IS NOT NULL
  AND f.created_at >= '2024-09-01'
  AND f.created_at < '2024-09-01' + INTERVAL 1 DAY
GROUP BY f.maybe_origin_model_token
ORDER BY usage_count DESC
LIMIT 10;
