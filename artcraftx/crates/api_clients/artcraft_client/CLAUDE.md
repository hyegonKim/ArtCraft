# artcraft_client

HTTP API client for Artcraft / Storyteller, including the API type definitions.

## Structure

- `api_defs/` — request/response type definitions, organized by feature:
  `users/`, `media_file/`, `jobs/`, `moderation/`, etc. (Formerly the separate
  `artcraft_api_defs` crate; merged here since this repo has no server to share
  them with.)
- `endpoints/` — one function per HTTP endpoint
- `credentials/` — session/cookie plumbing
- `datatypes/`, `error/`, `recipes/`, `utils/` — support code

## Conventions

- Request types: `#[derive(Deserialize)]`
- Response types: `#[derive(Serialize)]`
- Path params: `#[derive(Deserialize)]` with `PathInfo` suffix

## Dependencies

Keep `api_defs/` light: only `serde`, `serde_derive`, `chrono`, `url`, and
internal schema crates (`enums`, `tokens`).
