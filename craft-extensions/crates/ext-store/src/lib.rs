//! Desktop half of Craft extensions: the shared store on disk.
//!
//! Invariants:
//! - The launcher (and an app's "Install from file") writes; apps otherwise only read.
//! - An install is all or nothing: a package is unpacked and checked in a staging folder, then moved
//!   into place, and the state file is replaced atomically with a bumped generation counter.
//! - This crate never opens a network connection.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod location;
pub mod reader;
pub mod state;
pub mod store;
#[cfg(test)]
pub(crate) mod testutil;
pub mod unpack;
