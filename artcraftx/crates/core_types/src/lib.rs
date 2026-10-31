//! The highest-level shared types: enums and identifiers used across the
//! whole workspace (commands, credentials, the tasks database, API clients).
//!
//! This crate must stay dependency-light — everything else depends on it.

#[macro_use]
extern crate serde_derive;

pub mod enums;
#[macro_use]
pub mod identifiers;
