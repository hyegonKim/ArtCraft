//! enums
//!
//! The purpose of this library is to have a strongly-typed MySQL enum-type wrapper.
//! This should also work for CHAR/VARCHAR fields that work similarly to enums (typically
//! as part of a composite key)
//!
//! These types should also be friendly for API usage in JSON payloads.
//!
//! In the future this should be *CODEGEN DRIVEN* and should get checked into source control.
//!

#[macro_use]
mod macros;

#[cfg(test)] pub mod test_helpers;

pub mod by_table;
pub mod common;
pub mod error;
pub mod no_table;
pub mod tauri;
pub mod api_safe;
