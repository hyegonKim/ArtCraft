-- The upload-dedup ledger: which remote service already holds a copy of a
-- local file, per account. Before uploading, callers look up
-- (service, account, content hash) and reuse the service's id for the
-- asset instead of pushing the bytes again.
--
-- `service`     - which remote: "artcraft", "higgsfield", ...
-- `account_id`  - the credential the upload was made with (a credential id
--                 for stored accounts; a session fingerprint for ArtCraft
--                 web sessions). Different accounts never share uploads.
-- `service_id`  - the service's own primary key for the asset (ArtCraft media
--                 file token, Higgsfield media id, ...).
-- `service_url` - where the asset can be reached remotely, when the service
--                 publishes one (Higgsfield's CDN URL, ArtCraft's CDN URL).
--
-- Rows are advisory: a reuse is verified against the service first, and any
-- failure falls back to a fresh upload that overwrites the row.

CREATE TABLE asset_uploads (
  service              TEXT NOT NULL,
  account_id           TEXT NOT NULL,
  file_hash_blake3     TEXT NOT NULL,

  service_id           TEXT NOT NULL,
  service_url          TEXT,

  -- Size of the bytes that were uploaded, for sanity checks / display.
  file_size_bytes      INTEGER,

  uploaded_at          TEXT NOT NULL,
  -- Last time a caller reused this row instead of uploading.
  last_reused_at       TEXT,

  PRIMARY KEY (service, account_id, file_hash_blake3)
);

-- Reverse lookups (which local content is behind a service id).
CREATE INDEX idx_asset_uploads_service_id ON asset_uploads (service, service_id);
