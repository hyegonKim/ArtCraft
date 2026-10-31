-- The content-hash index of local files the app has seen.
--
-- One row per absolute file path. `file_size_bytes` + `file_mtime_ms` exist
-- so lookups can trust the cached hash without re-reading the file: if the
-- stat matches, the hash is current; if not, rehash and update.
--
-- `has_thumbnail` / `has_animated_preview` are hints — the thumbnail cache
-- directory on disk is the source of truth (files are keyed by
-- `{file_hash_blake3}.jpg` / `.webp` and regenerate if missing).

CREATE TABLE local_files (
  -- Absolute local file path.
  file_path            TEXT PRIMARY KEY NOT NULL,

  -- Lowercase extension-ish type: "mp4", "png", "jpg", ...
  file_type            TEXT,

  -- Stat fields captured when the hash was computed (invalidation).
  file_size_bytes      INTEGER NOT NULL,
  file_mtime_ms        INTEGER NOT NULL,

  -- BLAKE3 content hash, 64 lowercase hex characters.
  file_hash_blake3     TEXT NOT NULL,

  -- A static thumbnail jpg exists in the cache for this hash.
  has_thumbnail        INTEGER NOT NULL DEFAULT 0,

  -- A ~1s animated preview webp exists in the cache for this hash.
  has_animated_preview INTEGER NOT NULL DEFAULT 0,

  created_at           TEXT NOT NULL,
  updated_at           TEXT NOT NULL
);

-- Content-addressed lookups (dedup; the future upload ledger joins on this).
CREATE INDEX idx_local_files_hash_blake3 ON local_files (file_hash_blake3);
