//! sqlite_queries
//!
//! The purpose of this library is to contain all of our SQlite-related queries.
//! Keeping them in a single place abstracts away the data layer and makes it easy
//! to statically generate bindings and write tests.
//!

// Never allow these
#![forbid(private_bounds)]
#![forbid(private_interfaces)]
#![forbid(unused_must_use)] // NB: It's unsafe to not close/check some things

//// Okay to toggle
//#![forbid(unreachable_patterns)]
//#![forbid(unused_imports)]
//#![forbid(unused_mut)]
//#![forbid(unused_variables)]

// Always allow
#![allow(dead_code)]
#![allow(non_snake_case)]

pub mod queries;