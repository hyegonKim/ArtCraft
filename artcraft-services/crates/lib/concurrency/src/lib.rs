//! concurrency
//!
//! The purpose of this crate is to collect concurrency primitives not provided
//! in stdlib or other 3rd party crates.
//!
//! This name is okay to reuse since the Cargo "concurrency" crate is
//! name-squatted.
//!

// Never allow these
#![forbid(private_bounds)]
#![forbid(private_interfaces)]
#![forbid(unused_must_use)] // NB: It's unsafe to not close/check some things

//// Okay to toggle
#![forbid(unreachable_patterns)]
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

// Always allow
#![allow(dead_code)]
#![allow(non_snake_case)]

pub mod async_thread_kill_signal;
pub mod relaxed_atomic_bool;