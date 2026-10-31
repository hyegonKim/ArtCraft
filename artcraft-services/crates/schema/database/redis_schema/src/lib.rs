//! redis_schema
//!
//! Centrally implemented and documented Redis keys and Redis payloads.
//! This allows using the same Redis schema across microservice boundaries,
//! and makes updating and understanding the schema easier from a single
//! point of discovery.
//!

// Never allow these
#![forbid(private_bounds)]
#![forbid(private_interfaces)]
#![forbid(unused_must_use)] // NB: It's unsafe to not close/check some things

// Okay to toggle
#![forbid(unreachable_patterns)]
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

// Always allow
#![allow(dead_code)]
#![allow(non_snake_case)]

// Internal use only
#[macro_use]
pub (crate) mod macros;

pub mod keys;
pub mod payloads;
pub mod traits;
