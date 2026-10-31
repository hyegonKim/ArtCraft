//!
//! Common HTTP Server components
//!

#[macro_use] extern crate serde_derive;

// This is a little messy since we're supporting two versions of the Actix crate ecosystem.
// Long story short: one service requires an old Tokio runtime, and another needs the newer one.
//
// I don't think this will be too hard to maintain. AFAICT, the only major differences are which
// package "HttpResponseBuilder" is located in.

pub mod endpoints;
pub mod error;
pub mod request;
pub mod response;
pub mod util;
