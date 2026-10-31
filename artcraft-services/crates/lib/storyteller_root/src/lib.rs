//! storyteller_root
//!
//! This crate defines functions that locate the development root for Storyteller projects,
//! as well as our major monorepos. This can be used to build relative paths that should work
//! across developer machines and environments.
//!
//! These functions should not be relied upon in production.
//!
//! The following environment variables are read:
//!
//!   - STORYTELLER_ROOT
//!   - STORYTELLER_FRONTEND_ROOT
//!   - STORYTELLER_ML_ROOT
//!   - STORYTELLER_RUST_ROOT
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

pub use paths::seed_tool_data_root::get_seed_tool_data_root;
pub use paths::storyteller_frontend_root::get_storyteller_frontend_root;
pub use paths::storyteller_ml_root::get_storyteller_ml_root;
pub use paths::storyteller_root::get_storyteller_root;
pub use paths::storyteller_rust_root::get_storyteller_rust_root;
pub use substituted_path::get_substituted_path;

mod paths;
mod substituted_path;

