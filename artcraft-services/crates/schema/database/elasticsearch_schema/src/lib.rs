//! elasticsearch_schema
//!
//! The purpose of this library is to keep a list of schemas for our ElasticSearch documents
//! as well as index definitions.
//!

// Never allow these
#![forbid(private_bounds)]
#![forbid(private_interfaces)]
#![forbid(unused_must_use)] // NB: It's unsafe to not close/check some things

// Okay to toggle
//#![forbid(unreachable_patterns)]
//#![forbid(unused_imports)]
//#![forbid(unused_mut)]
//#![forbid(unused_variables)]

// Always allow
#![allow(dead_code)]
#![allow(non_snake_case)]

pub mod documents;
pub mod searches;
pub mod traits;
pub mod utils;
