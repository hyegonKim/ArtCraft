-- Thumbnail generation bookkeeping, so the background worker can find work
-- with one query instead of stat-ing the cache directory per file.
--
-- `thumbnail_state`:
--   'pending'     - never generated (or the file's content changed)
--   'generated'   - the jpg (and, for videos that allow it, the webp) exist
--   'unsupported' - the container/codec can't be thumbnailed; don't retry
--   'failed'      - generation errored; retried until `thumbnail_attempts`
--                   reaches the worker's cap
--
-- The cache file paths are recorded so readers (the task queue, the
-- frontend) never have to derive them; they are content-addressed:
-- `cache/thumbnails/{file_hash_blake3}.jpg` / `.webp`.

ALTER TABLE local_files ADD COLUMN thumbnail_state TEXT NOT NULL DEFAULT 'pending';
ALTER TABLE local_files ADD COLUMN thumbnail_attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE local_files ADD COLUMN thumbnail_last_error TEXT;
ALTER TABLE local_files ADD COLUMN thumbnail_jpg_path TEXT;
ALTER TABLE local_files ADD COLUMN animated_preview_webp_path TEXT;
ALTER TABLE local_files ADD COLUMN thumbnail_updated_at TEXT;

-- The worker's "what needs doing" scan.
CREATE INDEX idx_local_files_thumbnail_state ON local_files (thumbnail_state);
