//! A client for Higgsfield's consumer web API (`fnf-api-gw.higgsfield.ai`) —
//! the same endpoints the higgsfield.ai web app calls.
//!
//! Requests authenticate with a short-lived Clerk JWT (see
//! [`credentials::higgsfield_auth::HiggsfieldAuth`]). Each endpoint binding
//! under [`endpoints`] takes an `{Endpoint}Args { request, auth, host }`.
//!
//! The easy path is [`session::higgsfield_session::HiggsfieldSession`]: build
//! it from the browser's cookies once, and it mints / refreshes the JWT for
//! you and exposes every endpoint as a method.

#[cfg(test)]
pub(crate) mod test_utils;

pub mod client;
pub mod credentials;
pub mod endpoints;
pub mod error;
pub mod session;
pub mod types;
