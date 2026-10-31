//! Cookie-based web login windows for third-party sites.
//!
//! The frontend triggers a login for a [`crate::credentials::login_website::LoginWebsite`]
//! (via `open_web_login_command`). This module opens a fresh, cookie-cleared
//! Tauri webview and drives it through the site's [`login_journey::LoginJourney`]
//! — optional pre-navigation referrer, website entry page, then the login
//! page (fixed or discovered on-page) — see [`open_login_window`]. A
//! background thread ([`login_window_thread`]) then watches until the user
//! finishes signing in. Captured cookies (plus any username/email decoded
//! from a cookie JWT) are saved to the credentials directory as a cookie
//! [`crate::credentials::auth_credential::AuthCredential`].
//!
//! Each supported site implements [`login_window_trait::LoginWindowSite`]
//! under [`logins`], describing its journey and completion heuristics.

pub mod login_journey;
pub mod login_window_thread;
pub mod login_window_trait;
pub mod logins;
pub mod open_login_window;
pub mod utils;
