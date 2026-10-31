-- How the service came to hold the content. `upload`: we sent the bytes.
-- `generation_result`: the service produced the file itself (a generation
-- we downloaded), so `service_id` is its job id and the content can be
-- referenced as a previous generation without ever uploading it.

ALTER TABLE asset_uploads ADD COLUMN origin TEXT NOT NULL DEFAULT 'upload';
