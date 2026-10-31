# core_types — the highest-level shared types

The whole workspace depends on this crate; it must stay dependency-light.

- `enums/generation_source.rs` — `GenerationSource`: where a generation comes from (the service +
  auth mechanism behind a credential, and the provider recorded for tasks). Stored in credential
  TOML files and the sqlite tasks database — NEVER change existing string values.
- `identifiers/` — the concrete strongly-typed id types. Every non-database entity id gets a
  distinct type here (`CredentialId`, …). The *machinery* (minting, validation, the `define_id!`
  macro) lives in `identifiers/utils/id_core.rs`.

Database identifiers live elsewhere: `sqlite_identifiers` for the local tasks database, `artcraft_client::tokens`
for the server API.

## The rule: NEVER stringly-typed ids

A bare `String`/`&str` id is a bug. Function signatures, struct fields, file formats, and request
types all use the typed id. This makes "passed a `CredentialId` where another id was expected" a
**compile error**, not a production incident.

```rust
CredentialId::generate()                  // mint a new one
"credential_01j9…".parse::<CredentialId>()?  // validating parse of untrusted input
CredentialId::from_trusted(s)             // wrap a known-good string (a value we minted) — no validation
credential_id.as_str()                    // borrow the underlying string
```

## Adding a new id type

One line, alphabetized within its domain group:

```rust
define_id!(pub struct WidgetId => "wgt");
```

- Pick a short, collision-free prefix.
- Group types under a plain `//` section comment.
- Add a test only when the type carries unusual rules; the generic tests already in `lib.rs`
  cover the macro itself.

## Trusted vs. validated — pick deliberately

- Crossing a trust boundary (Tauri request payload, hand-written file)? → `parse()` / `from_str`.
- Value we produced and are round-tripping (a credential file we wrote)? → `from_trusted` (cheaper).
  NB: existing credential files minted before the ULID switch carry shorter entropy — they load via
  serde/`from_trusted` and stay valid; only strict `parse()` would reject them.

When unsure, validate.

## The machinery (`src/id_core.rs`)

- `mint(prefix) -> String` — `{prefix}_{26-char lowercase Crockford ULID}`. Time-ordered + globally
  unique. This is the ONLY place these ids are minted.
- `validate(s, prefix) -> Result<(), ParseIdError>` — checks the `{prefix}_` and a 26-char ULID body.
- `ParseIdError` — `WrongPrefix` / `BadUlid`, returned by every `FromStr`.
- `define_id!` — stamps out a `#[serde(transparent)]` newtype over `String` with `generate()`,
  validating `FromStr`, `from_trusted()`, `as_str()`/`into_string()`/`AsRef<str>`/`Display`/`Debug`.
  No sqlx integration (these ids live in files, not database columns); add it if that ever changes.
