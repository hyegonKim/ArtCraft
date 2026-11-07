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

### Verified staging deployment — 2026-11-07

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
- These follow-up checks are local. The Netlify deployment above was retained;
  its main URL, deploy permalink, and bundle still returned 401 anonymously.
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
