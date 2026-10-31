//! user_traits_component
//!
//! Expose some traits for other plugins / components.
//! This will probably go away once we start consolidating the plugins into the main service.
//!

// Never allow these
#![forbid(private_bounds)]
#![forbid(private_interfaces)]
#![forbid(unused_must_use)] // NB: It's unsafe to not close/check some things

// Okay to toggle
#![forbid(unreachable_patterns)]
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![allow(unused_variables)] // NB: issue with automock

// Always allow
#![allow(dead_code)]
#![allow(non_snake_case)]

pub mod traits;
