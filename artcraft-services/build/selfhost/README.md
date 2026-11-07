# Self-hosting with Docker Compose

This directory runs the ArtCraft backend (`storyteller-web`) and the services it
depends on with Docker Compose.

| Service           | Image                    | Purpose                                     |
|-------------------|--------------------------|---------------------------------------------|
| `storyteller-web` | built from this repo     | HTTP API, on `http://localhost:12345`       |
| `mysql`           | `mysql:8.4`              | Accounts, media metadata, jobs, billing     |
| `migrate`         | `mysql:8.4`              | Applies `_database/sql/migrations`, exits   |
| `redis`           | `redis:7`                | Caching, rate limiting, job progress        |
| `elasticsearch`   | `elasticsearch:8.15.3`   | Search                                      |
| `seaweedfs`       | `chrislusf/seaweedfs`    | S3-compatible storage for media             |
| `storage-init`    | `chrislusf/seaweedfs`    | Creates the storage buckets, exits          |

Only the API port (`12345`) is published to the host.

## Requirements

- Docker with Docker Compose v2.23 or newer.
- About 8 GB of memory for Docker and 20 GB of free disk. The first build
  compiles the Rust workspace in release mode and can take a long time.

## Usage

From this directory:

```bash
cp .env.example .env    # then edit the passwords and secrets
docker compose up --build -d
```

Check that the server is up:

```bash
curl http://localhost:12345/_status
```

Stop everything with `docker compose down`. Add `-v` to also delete the
database, search index and stored media.

## Configuration

Settings live in `.env`, which git ignores. `docker-compose.yml` sets the
connection settings for MySQL, Redis, Elasticsearch and storage, so you only
need to change passwords and secrets.

`storyteller-web` requires credentials for every third-party provider (FAL,
OpenAI, Stripe, Resend and others) to be present at startup. `.env.example`
sets them to the placeholder `unset`, which lets the server start. Features
that call a provider only work after you add a real key for it.

Provider webhooks, such as `FAL_WEBHOOK_URL`, must be reachable from the
internet for asynchronous generation results to arrive.

## Database migrations

The `migrate` service applies every migration in `_database/sql/migrations` in
order and records each one in `__diesel_schema_migrations`, the table the diesel
CLI uses. It runs on every `docker compose up` and only applies migrations that
have not run yet.

## Connecting the web app

To point the browser app at this server, run it from `frontend` as described in
the repository README:

```bash
VITE_USE_LOCAL_API=true npx nx dev artcraft-webapp
```

### Private staging authentication

Use a separate deployment directory and `.env` for staging. Set
`ARTCRAFT_WEBAPP_EXTRA_ORIGIN` to the exact protected web app origin and
`ARTCRAFT_GOOGLE_CLIENT_ID` to the staging Google web client ID. Generate
independent database passwords and signing secrets, and set `COOKIE_SECURE=true`
and `COOKIE_DOMAIN` to the staging API hostname. Build the web app with
`VITE_ARTCRAFT_API_ORIGIN` set to the HTTPS API origin and `VITE_GOOGLE_CLIENT_ID`
set to the same client ID.

```bash
docker compose -f docker-compose.yml -f docker-compose.staging.yml config -q
docker compose -f docker-compose.yml -f docker-compose.staging.yml up --no-build -d
curl --fail http://127.0.0.1:12346/_status
```

The staging overlay requires Compose 2.24.4 or newer. Build the image on a
separate machine and import it before running the overlay. Set
`ARTCRAFT_STAGING_IMAGE` in the staging `.env` to that local image tag.
The overlay uses a separate project and volumes. It starts MySQL, migrations,
Redis, and the API with memory limits for the authentication smoke test.
Search and media are unavailable. Check migration completion and measure
actual memory use during sign-up and sign-in before enabling an HTTPS route.

The runtime image used in the staging verification was packaged with
[Dockerfile.staging-runtime](./Dockerfile.staging-runtime). Its build context
contains the Linux `storyteller-web` binary, the repository's `includes/`
directory, and the public `storyteller-web.common.env`,
`storyteller-web.production.env`, and `storyteller-web.development.env` files.
Supply deployment secrets through the separate Compose `.env` file.

```bash
# Run on the Linux build host after cargo build --locked --bin storyteller-web.
mkdir -p runtime-image
cp target/debug/storyteller-web runtime-image/
cp -a includes runtime-image/
cp crates/service/web/storyteller_web/config/storyteller-web.{common,production,development}.env runtime-image/
docker build -f build/selfhost/Dockerfile.staging-runtime -t artcraft-services:auth-staging-678be91 runtime-image
```

The API binds to VPS loopback. On a shared VPS, use a dedicated virtual host
in the existing HTTPS proxy. Expose only the staging authentication endpoints
with appropriate request limits.

The deployed Traefik configuration is in
[docker-compose.staging-https.yml](./docker-compose.staging-https.yml).
It uses the existing `websecure` entrypoint and `letsencrypt` resolver.
After internal authentication checks pass, apply it with the other two files.

```bash
docker compose -f docker-compose.yml -f docker-compose.staging.yml -f docker-compose.staging-https.yml config -q
docker compose -f docker-compose.yml -f docker-compose.staging.yml -f docker-compose.staging-https.yml up -d --no-build --no-deps storyteller-web
```

The router's Origin match and CORS are browser request controls.
Netlify project visibility supplies the site access gate.
The HTTPS router also restricts paths and methods and limits request rates.

Keep Netlify visitor protection enabled for both the main staging URL and
previews. In the staging CSP, allow only the necessary authentication API URLs
and the [Google Identity Services URLs](https://developers.google.com/identity/gsi/web/guides/get-google-api-clientid#content_security_policy).
Check that anonymous requests to Netlify still return 401 and that model
selection still uses the local preview catalog with Create disabled.

### Private staging cloud storage

The optional [media overlay](./docker-compose.staging-media.yml) extends the
authentication-only setup on the same VPS. Apply it **last**. It enables
SeaweedFS and bucket initialization without enabling Elasticsearch or workers.
The existing Rust API continues to own accounts, SQL metadata, and project writes;
the [Python gateway](./staging-media-gateway.py) protects the legacy media routes.

Set `MEDIA_LINK_SECRET` once in the deployment-only `.env` to at least 32 random
bytes (for example, a 64-character value generated by `openssl rand -hex 32`).
Retain it across normal deployments; never put it in frontend variables or Git.
Keep `ARTCRAFT_WEBAPP_EXTRA_ORIGIN` set to the protected staging origin.

```bash
docker compose -f docker-compose.yml -f docker-compose.staging.yml -f docker-compose.staging-https.yml -f docker-compose.staging-media.yml config -q
docker compose -f docker-compose.yml -f docker-compose.staging.yml -f docker-compose.staging-https.yml -f docker-compose.staging-media.yml up -d --no-build seaweedfs storage-init storyteller-web media-gateway
python3 -m unittest discover -s build/selfhost -p test_staging_media_gateway.py
```

- The gateway runs without third-party Python packages, as a non-root user with
  a read-only root filesystem, dropped capabilities, and a 320 MiB memory limit.
  SeaweedFS has a 384 MiB limit. Its master grows one volume at a time, with
  eight volume slots and a 512 MiB limit for newly created volumes. The default
  seven-volume growth batch cannot fit a one-volume staging configuration.
- SeaweedFS's volume data and `/data/filerldb2` metadata live in `seaweedfs-data`.
  No S3, filer, master, or volume-server ports are published on the host. MySQL
  continues using the existing persistent volume. Back up both stores together;
  a container restart check does not establish disaster-recovery coverage.
- Uploads, project lists, and metadata requests require an ArtCraft session and
  the exact staging origin. Both cookies and the signed `session` header work.
  File reads, updates, renames, soft deletes, cover attachments, and batch reads
  check ownership. Session-only SQL lists replace the legacy global listing.
- Media bytes use 30-minute signed bearer URLs issued after authorization.
  The URL itself grants temporary access; never publish it or log its query.
  Filenames retain their extension so existing video/audio/3D viewers work.
  Downloads stream from the private filer and support `Range` requests.
- The gateway caps the **entire request body** at 64 MiB and limits concurrent
  requests. It exposes a narrow media allowlist and read-only folder/tag lists;
  generation, billing writes, search indexing, and folder/tag mutations are not
  enabled by this overlay.
- Build the browser app with `VITE_ARTCRAFT_LOCAL_EDITING=false`, the existing
  `VITE_ARTCRAFT_API_ORIGIN`, and `VITE_GOOGLE_CLIENT_ID`. Copy the staging CSP
  and routing files into the build output. Keep Netlify Production and Deploy
  Preview visibility Private. Revalidation headers prevent stale app entry
  pages from hiding a newly deployed editor fix.

Use the [production staging builder](../../script/website/unix_frontend_webapp_staging_build.sh)
after tests instead of packaging a standalone Vite build. Test tasks can leave
development-JSX library artifacts in `dist`; packaging those with production
React caused the observed `jsxDEV is not a function` startup failure. The builder
forces `NODE_ENV=production`, rebuilds Nx dependencies without cache, checks for
incompatible development-JSX calls and missing Ammo assets, then packages the
staging headers, routing files, and source manifest. A regression check against
the actual broken bundle confirmed that ZIP creation is refused.

```bash
# From artcraft-services, with Node 22 and installed frontend dependencies.
VITE_ARTCRAFT_API_ORIGIN=https://artcraft-auth-staging.ncubestudio.cloud \
VITE_GOOGLE_CLIENT_ID=YOUR_PUBLIC_STAGING_GOOGLE_CLIENT_ID \
./script/website/unix_frontend_webapp_staging_build.sh /tmp/artcraft-private-staging.zip
```

Build a clean verified checkout. The manifest defaults to Git HEAD;
`ARTCRAFT_BUILD_COMMIT` is an explicit override for an exactly matched candidate
source tree. No signing secret is read by this browser build script.

Video projects autosave full documents and retry failed saves while the editor
remains open. The server identity and title override temporary/stale JSON
metadata on reopen. 3D scenes use their Save / Load my scene controls. Image
uploads can be reopened from the account library. The image editor also has
Save project / Save copy / Open project controls for editor_2d layer documents.
These use explicit manual saves and show unsaved changes or request failures. Existing browser-local
projects are not migrated automatically. This provides account-based persistence,
not live multi-tab merging or an offline upload queue.

### 2D layer-project verification — 2026-11-07

The image editor now stores versioned, portable `editor_2d` documents on the
existing API. No new VPS service, endpoint, port, or secret is required.

- Source: `b368683edad42586f4428aebbf351e180ee10c85`.
  Private deploy: `6aef03fd9278c7c4919c2856`; actual bundle: `assets/index-Bb0xhcFJ.js`.
- Save project creates a private row, then updates the same token. Save copy
  creates a separate row. Project names use the existing rename endpoint.
  Open project lists only the account's 2D documents with cursor pagination.
  The `?project=...` URL restores the saved document on reload.
- Base images and image layers are embedded without resampling. Temporary
  signed/blob links are not needed when reopening. Dimensions, ordered shapes
  and strokes, transforms, locks, paint/eraser blending, masks, and paint settings
  are retained. Every asset decodes before the current document is replaced.
- Save errors keep the dirty indicator and allow retry. New/Open use an in-app
  Keep editing / Discard changes dialog. Native browser confirmation had blocked
  the embedded validation browser and was replaced. Unsaved browser unload still
  uses the browser's normal beforeunload protection.
- Actual UI opened an API-seeded 1920 x 1080 document containing a shape, an image, a translucent brush stroke, and a mask. Shape deletion, Undo/Redo, Save copy, rename/update, reload, and New -> Open list restore passed. The final deployment restored the remaining image/brush order and one mask; API checks confirmed the deleted shape stayed absent, coordinates/settings persisted, and original image bytes matched. The final in-app Keep editing and Discard changes flows both passed.
- Actual PNG import, new project save, and reload restored the 1280 x 720 base
  image. Base and layer image bytes matched the original PNG exactly. Another
  account received 404 when reading the newly saved layer project.
- All 63 frontend test targets completed with 283 actual passing tests (valid Nx cache retained): webapp 65 and PageDraw 15. App TypeScript and the final uncached production webapp plus its 53 dependency build tasks passed.
- Production and Deploy Preview remain Private. Eight anonymous requests to the main/editor/project routes, final JavaScript, build manifest, and deploy permalink returned 401. The deployed build manifest matches source b368683e and the browser loaded the bundle above.

These are manual document saves, not autosave, offline queuing, or concurrent-tab
merge. Undo history, clipboard, selection, generation tasks, and external 3D-model
source archives are not project assets; rasterized 3D image layers retain their
pixels. The 64 MiB upload limit includes base64/JSON/multipart overhead. Back up
MySQL and SeaweedFS together as described above. No paid generation was run.

### Previous live cloud verification — 2026-11-07

- Verified source: `fbafc5501ff52eb26c4d3059a0cb9a953d877a10`.
  Private Netlify deploy: `6aeeee7743023190c3b66265`.
  Actual browser bundle: `assets/index-BnfSdNyv.js`; the deployed build manifest
  matches the source and enables server storage, with local editing disabled.
- All 66 frontend build targets passed. The final webapp and its 53 dependency
  tasks were rebuilt in production without Nx cache. App TypeScript passed.
  All 63 test targets completed, including 268 actual tests (valid Nx cache was
  retained); all 14 gateway tests passed. These counts exclude empty targets.
- Actual staging UI imported MP4, inserted it into the timeline, renamed the
  project, saved X=12, and restored it from the project list and after reload.
  The project and media survived a SeaweedFS restart. Export to the cloud library
  reopened and played; the downloaded AVC file was 64 x 64, 25 fps, 25 packets,
  and one second long. A controlled gateway outage showed a save-error toast;
  restarting it allowed the five-second autosave retry to persist X=13. X=12
  was restored and verified in both the final UI and the server document.
- A saved 3D scene restored camera X=0.25 after storage restart. A fresh final
  deployment tab retained login, loaded the scene, and uploaded a 1280 x 720
  Capture PNG. Its actual viewer image decoded at that resolution, and its
  metadata marked it as a visible, non-intermediate library file.
- PNG upload and account-library reopen passed, including Edit on Canvas and
  Pick from Library with Show uploads. The original image and history thumbnail
  decoded at 1280 x 720. Signed URLs are preserved in all three paths instead
  of appending a second query marker.
- Cold 3D startup previously discarded session responses after 1.5 seconds.
  Late successful responses now update the session; transient transport errors
  preserve established login, and superseded responses cannot undo logout.
  Four regression tests cover these cases, including a 15-second response.
- Real API checks passed original-byte SHA256 matching, video Range 206,
  create/update/list/rename/re-read for video_timeline, scene_3d, and editor_2d,
  cross-account read/write 404, anonymous request 401, wrong Origin 403,
  invalid signature 403, signed-session headers without cookies, and a real
  64 MiB + 1 byte upload rejected with 413.
- Production and Deploy Preview visibility remain Private. Nine anonymous
  requests to app routes, the final bundle, Ammo JS/WASM, build manifest, and
  deploy permalink returned 401. Issued media bearer URLs remain usable by
  their holder until their 30-minute expiry; this is distinct from Netlify auth.
- At that earlier deployment, layered 2D project Save/Open UI, automated migration of browser-local projects,
  conflict merging, disaster recovery, background workers, mail delivery, paid
  generation, and billing were not validated or added by this change.

### Previous local editor verification — 2026-11-07

The deployed app source is `ddc9da82e0b4e3d21730bac4a607d2fa990f0c28`.
Netlify deploy `6aeec364f5b4bc32e48a40e3` serves `assets/index-Ca42bwNO.js`.
The served build metadata matched that commit. Production and Deploy Preview
visibility both remain Private.

- Allow `data:` and `blob:` in staging `connect-src` for embedded Spark WASM
  and browser-local video restoration. Remote API origins/paths stay restricted.
- Emit the installed Three.js Ammo JS/WASM pair at `/jsm/libs/`. Initialize
  the runtime once using root-relative URLs and allow retries after failure.
- Keep a local download link for 3D captures even if gallery upload fails.
- Offer New project when video-project listing fails. This staging build sets
  `VITE_ARTCRAFT_LOCAL_EDITING=true`: video projects and source Files persist
  in browser IndexedDB, and exports download to disk. Server adapters remain
  the default in builds without this explicit option.
- Actual Chrome image-editor checks covered a Full HD blank canvas, a brush
  stroke, Undo, and Redo. 3D checks covered Cube insertion, transforms with
  Undo/Redo, Build/Record switching without losing the object, and a downloaded
  1280x720 PNG capture.
- Actual Chrome video checks covered new-project creation, MP4 import,
  preview/playback, trimming with Undo/Redo, renaming, disk export, and reload
  with the source media, timeline edit, and project name restored. The exported
  test MP4 was H.264, 64x64, 25fps, 13 frames, and 0.52 seconds. Only Save to
  disk was selected.
- All 63 Nx test targets completed with 243 actual tests passing. The affected
  3D and webapp suites passed 25 and 46 cases; webapp TypeScript and production
  library/webapp builds passed. Browser regression probes reproduced the old
  Spark/Blob failures and passed with the corrected CSP; emitted Ammo WASM
  initialized and computed vector (1, 2, 3).
- Eight anonymous requests returned 401: the main URL, all three editor URLs,
  the bundle, Ammo JS, Ammo WASM, and the new deploy permalink.

Cloud gallery uploads and server project synchronization remain unavailable
on the authentication-only backend. A 3D capture upload still reports failure;
its local PNG download succeeds. Paid generation, billing, and exhaustive
codec/browser compatibility were outside this verification.

### Earlier authentication redeployment — 2026-11-07

The protected frontend remains
[artcraft-web-private-staging.netlify.app](https://artcraft-web-private-staging.netlify.app/).
The deployed application source is commit
[`cbd1dc095f0107b32263dc8e4730bf48a7aa3618`](https://github.com/hyegonKim/ArtCraft/commit/cbd1dc095f0107b32263dc8e4730bf48a7aa3618).
Netlify deploy `6aeeae405ca34d4620ca48ee` was published at 13:51 KST.
Its web bundle is `assets/index-AeQiLn3A.js`; the served
`artcraft-build.json` commit and browser-loaded script matched the build.

- The production Vite build passed with the existing staging API origin,
  Google web client, CSP, and static routing configuration.
- Live Chrome checks created a new test email account, displayed the welcome
  screen, signed out, signed in with that email, and retained the session
  after reload.
- The official Google button opened account selection. Selecting the existing
  test account returned to `/pricing`; the user menu and the same signed-in
  account remained available after reload.
- All 22 image models and all 22 video models were individually selected.
  Each selection updated the picker and kept Create disabled. Entering a
  nonempty prompt on each page also kept Create disabled.
- Production visibility and Deploy Preview visibility both remained Private.
  Eight cookie-free requests returned 401: the main URL, `/signup`, `/login`,
  `/create-image`, `/create-video`, the new bundle, `/artcraft-build.json`,
  and the new deploy permalink.

This documentation-only update does not republish the app or change visitor
access. Test credentials and server secrets are excluded from the repository.
Paid generation, billing, email delivery, password-reset emails, and all
editors' GPU behavior were outside this deployment verification. Some separate
WASM initialization requests logged `Failed to fetch` on initial load; the
listed authentication and model-selection checks completed successfully.

### Initial staging deployment — 2026-11-07

The protected frontend is `artcraft-web-private-staging.netlify.app`.
The authentication API origin is
`https://artcraft-auth-staging.ncubestudio.cloud`.
The deployed web bundle is `assets/index-CgqxJjkJ.js` in Netlify deploy
`6aee8f4dd234b04876ca4ed6`.

- MySQL migrations completed, Redis returned PONG, and the API passed local
  CORS, email sign-up, signed-session, password-login, and invalid Google
  credential checks.
- Live Chrome checks completed email sign-up, logout, password sign-in,
  Google account selection and consent, and session persistence after reload.
- The main Netlify URL, deploy permalink, and JavaScript bundle returned 401
  without a Netlify session. Disallowed API origins and generation/admin paths
  returned 404.
- Both model pickers listed 22 models, applied another selection, and kept
  Create disabled.
- CORS regression tests passed 79 cases. Web authentication tests passed 26
  cases. The Linux binary and runtime image built successfully.
- The earlier GenHub `getStatus` project-graph error did not reproduce after
  an isolated dependency install and uncached graph generation. The follow-up
  clean `npm ci` and `nx sync:check` passed. All 66 Nx build targets passed
  with zero TypeScript declaration diagnostics; all 63 test targets completed,
  with 236 actual tests passing. Eighteen targets have no test files and one
  existing spec file contains no tests; these are not counted as test cases.
- Dashboard's TypeScript/Vite build and four tests passed. The standalone Next
  website's production build, typecheck, and 31 fixture-backed Playwright media
  tests passed. See the root README for the commands and source fixes.
- These follow-up checks were local. At that stage the initial deployment
  was retained; its main URL, deploy permalink, and bundle still returned 401
  anonymously.
  Rebuilding the changed libraries in production had fixed the earlier
  `jsxDEV is not a function` browser failure.

For a manual Netlify deployment, copy the tested staging files into the
webapp's publish directory after the production build.

```bash
cp apps/artcraft-webapp/staging/_headers apps/artcraft-webapp/dist/
cp apps/artcraft-webapp/staging/_redirects apps/artcraft-webapp/dist/
cp apps/artcraft-webapp/staging/_staging-unavailable.html apps/artcraft-webapp/dist/
```

These files configure CSP, popup compatibility, and static routing.
The Netlify project and preview visibility must also be Private.
Email delivery, password-reset emails, billing, and paid generation were
outside this verification.

## Limitations

- Only the HTTP API is included. Background workers (provider polling, video
  thumbnails, email, analytics) are not started.
- Elasticsearch starts with no indexes. Index definitions are in
  `_database/elasticsearch/index_definitions`.
