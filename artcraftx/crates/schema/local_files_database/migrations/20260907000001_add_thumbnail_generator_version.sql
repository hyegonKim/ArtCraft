-- Which generator produced a row's thumbnails. The worker regenerates rows
-- whose version is older than the current one, so output changes (size,
-- duration, quality) roll out to existing cache entries without anyone
-- deleting the cache.

ALTER TABLE local_files ADD COLUMN thumbnail_generator_version INTEGER NOT NULL DEFAULT 0;
