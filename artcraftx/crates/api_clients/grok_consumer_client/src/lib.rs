//! A Grok API client.

#[cfg(test)]
pub (crate) mod test_utils;

// Library utils
pub mod client;

// User lib
pub mod credentials;
pub mod datatypes;
pub mod error;
pub mod recipes;
pub mod endpoint_bindings;
pub mod prompt_flags;
pub mod utils;
