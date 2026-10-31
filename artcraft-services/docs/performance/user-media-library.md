# User media library query

`GET /v1/media_files/list/user/{username}` runs an exact count before fetching
each page. The webapp continues to use it for infinite scroll. MySQL can choose
an index-merge plan that intersects the creator index with global indexes on
NULL deletion timestamps and the intermediate-file flag. That makes the count
scan unrelated users' index entries.

The queries now force the creator index for counts and the creator/date index
for page selection. A limited derived table selects IDs before joining prompt,
cover, model, and stats data. Ordering uses `(created_at, id)` to break timestamp
ties consistently. Shared predicates preserve author/moderator/public visibility,
deleted-file exclusions, uploads, and class/type/engine filters.

The legacy `GET /v1/media_files/list/user/{username}` contract is unchanged:
original query fields and defaults, no new page-size cap, and the original
`pagination: { current, total_page_count }` with a numeric total. Its historical
`1 + floor(count / page_size)` calculation is intentionally preserved, including
exact multiples. It always performs the optimized exact count.

The separate `GET /v1/media_files/list_v2/user/{username}` endpoint is available
for adoption after backend deployment. `list_v2` fetches one extra record
and returns `pagination: { current, has_more }`. It never counts the library and
has no count-mode flag or total-page field. `list_v2` accepts page sizes from 1 to 100.
Both endpoints return the same media-item fields and enforce the same visibility
and media filters.

Skipping the count is the additional speed improvement: even the optimized
legacy count must visit all matching rows before returning an exact total. The
new endpoint avoids that work on every request, including the first page. For a
60-item page it fetches at most 61 result records; it can still scan additional
index entries to apply filters or skip an offset. Compatible index-selection
and deferred-join improvements remain shared by both endpoints.

Pagination remains offset based, so very deep pages still scan preceding
matching entries. `list_v2` removes the full-library count from infinite-scroll
requests, but does not remove that offset cost.

## Deployment

The frontend switch is deferred. The webapp continues using
`GalleryModalApi.listUserMediaFiles` and the existing
`/v1/media_files/list/user/{username}` endpoint with numbered pagination.
The `UserMediaFilesV2Api` library binding and its tests are retained, with no live
frontend callers.

Deploy and verify storyteller-web with the new `list_v2` route before a separate
frontend change adopts `UserMediaFilesV2Api` and its `has_more` pagination.
Both routes remain in API v1; this is an endpoint-local revision. Existing
clients and the original bindings remain unchanged.

No new migration is needed. The existing migration
`2026-07-03-074740-0000_alter_media_files_add_user_created_at_index`
must already be applied: both page queries require `idx_creator_created_at`.
The legacy count query uses the longstanding `fk_maybe_creator_user_token` index.

Endpoint logs distinguish legacy/list_v2 and report `page_ms`; the legacy route
also reports `count_ms`.
Use those timings to verify the actual production improvement after deployment.

## Local validation

Database tests use `IsolatedTestDatabase`: a fixed `/tmp/mysql.sock`, new random
schemas, and checked-in migrations. They do not read production connection URLs
or modify existing schemas. From the repository root:

```sh
SQLX_OFFLINE=true cargo test -p mysql_testing --test user_media_library
SQLX_OFFLINE=true cargo test -p mysql_testing --test user_media_library -- --ignored --nocapture
```

The large fixture creates 300,000 rows, 60,000 owned by the target account. On
local MySQL 8.4.11, the production page query returned 61 rows in 3.4 ms with 91
sequential index reads. Its test asserts bounded reads rather than wall-clock
latency. The legacy user-scoped count took 38.5 ms.

A separate local EXPLAIN ANALYZE comparison with that distribution measured the
old count at 166 ms and the forced creator-index count at 41.5 ms. The old plan
scanned 300,000 entries in each of three global indexes. These are synthetic
local measurements, not production response-time promises.

Functional fixtures cover public/private/hidden visibility, other owners,
deleted and intermediate files, all library media classes including legacy 3D,
upload/type/engine filters, same-timestamp pagination, joined metadata, unknown
usernames, and caller-owned transactions.

The real Actix route test uses signed sessions and disposable MySQL fixtures to
lock down the original request defaults, response keys/types, exact-multiple page counts,
and page sizes above 100. It also checks `list_v2` first/last/empty pages, validation,
media-item parity, filters, and private-file access:

```sh
SQLX_OFFLINE=true cargo test -p storyteller-web --bin storyteller-web user_media_endpoint_versions
```

Frontend HTTP-binding tests:

```sh
cd frontend
node_modules/.bin/vitest run --config libs/api/vite.config.ts src/lib/MediaFilesApi.spec.ts
```
