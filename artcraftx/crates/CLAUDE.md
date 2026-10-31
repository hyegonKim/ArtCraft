# Rust Workspace

All Rust code lives under `crates/`. The workspace is defined in `Cargo.toml` at the repo root.

## Code Style

- Two spaces for indentation (not four)
- No minimum supported Rust version; use latest stable features freely
- Never use `println!` or `eprintln!` outside of tests; use `log` crate macros instead
- Use `anyhow::Result` / `AnyhowResult` for fallible functions in application code
- Fields that are optional use the `maybe_` prefix: `maybe_creator_user_token`
- When two crates export the same type name, alias with a suffix: `use foo::Bar as BarFoo;`
- Prefer `use` imports over inline fully-qualified paths; only qualify inline for true one-offs or std prelude collisions (`Result`, `Option`, `Error`)

## File Layout

Organize for top-to-bottom reading. Important things first, details later.

1. **Constants** at the top (after imports)
2. **Primary public type** (the main struct/enum the file defines)
3. **Supporting types** below the primary type
4. **API types** in order: Request, Response, Error
5. **`impl` blocks** after their type definitions
   - Constructors first, then public methods, then private helpers
   - Callers above callees — follow the call chain downward

## Test Layout

Within `#[cfg(test)] mod tests { ... }`:

1. **Imports**
2. **Constants** (test IDs, URLs, fixture data) — small context that tests reference
3. **Test functions or sub-modules** grouped by category
4. **Helper functions** (builders, pipeline runners, assertions)

Constants provide small context up front. Test cases come next so engineers see *what* is
tested before *how* the helpers work.

Group related tests into sub-modules (`mod pricing_tests { ... }`) when a category has 2+
tests. Don't wrap a single test in its own module. Keep nesting to two levels max. Shared
helpers go in the parent `mod tests` so sub-modules can `use super::*`.

## Building

- Most crates: `cargo check -p {crate_name}` or `cargo test -p {crate_name}`
- Crates using SQLx (eg. `sqlite_database`): `SQLX_OFFLINE=true cargo check -p {crate_name}`

## Key Crates

- `artcraftx` (desktop) — Tauri desktop app
- `router` — provider routing for image/video generation (Artcraft, Fal, Seedance2Pro)
- `artcraft_client` — HTTP client for the Artcraft/Storyteller API, including the API type definitions (`artcraft_client::api_defs`), enum variants stored as strings (`artcraft_client::enums`), and opaque server-minted identifier tokens with Stripe-like prefixes (`artcraft_client::tokens`, e.g. `user_`, `jinf_`)
- `seedance2pro_client` — HTTP client for the Kinovi/Seedance2Pro video generation service
- `core_types` — highest-level shared types: `GenerationSource`, the desktop app's own IDs (e.g. `credential_{ulid}`)
- `sqlite_database` — SQLx queries + embedded migrations for the desktop app's local task database
- `local_files_database` — separate SQLite DB (runtime SQLx queries, no macros): BLAKE3 content-hash index of local files + thumbnail accounting
- `thumbnails` — pure-Rust thumbnail generation: images via `image`, H.264 MP4 first-frame JPEG + ~1s animated WebP via `mp4`/`openh264`/`webp-animation`
- `file_hashing` — streaming BLAKE3 content hashing
- `sqlite_identifiers` — enums and IDs used in sqlite queries as enums or keys
