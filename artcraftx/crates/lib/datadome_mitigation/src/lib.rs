//! datadome_mitigation
//!
//! What to do about DataDome's bot protection. The errors live in
//! `datadome_errors`; this crate holds the counter-measures shared by API
//! clients and the desktop app:
//!
//! - [`client_id`] — DataDome trusts a request when its `datadome` cookie
//!   and `x-datadome-clientid` header agree with the browser fingerprint.
//!   Derive the header from the cookie so replayed sessions send both.
//! - [`fingerprint`] — replay captured cookies under the User-Agent (and
//!   browser-family fingerprint) that earned them.
//! - [`strategy`] — map a [`DataDomeError`](datadome_errors::datadome_error::DataDomeError)
//!   to a [`DataDomeMitigation`](strategy::datadome_mitigation::DataDomeMitigation).

pub mod client_id;
pub mod fingerprint;
pub mod strategy;
