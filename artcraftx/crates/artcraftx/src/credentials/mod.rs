//! Credential storage for third party services.
//!
//! Credentials live as individual TOML files in the app's credentials
//! directory (by default `~/Artcraft/artcraftx/credentials`). Users may
//! hand-write files there under any file name; the app periodically lists
//! and loads them all, and can rewrite managed files in place (e.g. to
//! refresh cookies or record success/failure timestamps).
//!
//! There are two layers:
//!
//! 1. [`credential_toml::CredentialToml`] — the tolerant serialization
//!    layer (the on-disk TOML schema). Everything optional so hand-written
//!    files parse.
//! 2. [`auth_credential::AuthCredential`] — the validated in-app layer the
//!    app shuttles around to authenticate. Exactly one secret (cookie XOR
//!    api key), tied back to its source file path.

pub mod api_key_credential;
pub mod auth_credential;
pub mod cookie_credential;
pub mod cookie_credential_grok_extra_pieces;
pub mod credential_toml;
pub mod credential_user_info;
pub mod find_service_credentials;
pub mod login_website;
pub mod service_cookie_origin;
