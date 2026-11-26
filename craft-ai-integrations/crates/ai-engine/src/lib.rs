#![doc = include_str!("../../../docs/ADD-AN-ENGINE.md")]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod job;
mod provider;
mod registry;
mod runner;
pub mod testing;

pub use provider::EngineProvider;
pub use registry::EngineRegistry;
pub use runner::{LoadedModel, ModelRunner, RunContext};
