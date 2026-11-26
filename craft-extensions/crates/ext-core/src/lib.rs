//! Package contract for Craft extensions (themes, plug-ins, resources).
//!
//! Pure data and validation: no file system, no network, no clock. It builds for
//! `wasm32-unknown-unknown`. Everything that touches disk lives in `craft-ext-store`.
//!
//! Invariants:
//! - Every byte that reaches this crate is hostile: sizes are capped before parsing, ids and paths are
//!   checked by character class, and nothing here indexes with an untrusted number.
//! - A package never states its own trust. The tier is assigned by the host from where the package
//!   came, never read from the package.
//! - The third-party path is behind the `third-party` cargo feature, off by default.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod id;
pub mod index;
pub mod manifest;
pub mod palette;
pub mod path;
pub mod text;
pub mod tier;
pub mod version;

#[cfg(test)]
mod gate_tests {
    /// The third-party path must never be on by default: a release build would then accept outside
    /// publishers before the security gates have passed (docs/THREAT-MODEL.md, G5).
    #[test]
    fn third_party_feature_is_not_a_default_feature() {
        let manifest = include_str!("../Cargo.toml");
        let features = manifest.split("[features]").nth(1).unwrap_or("");
        let default_line = features.lines().find(|l| l.trim_start().starts_with("default"));
        if let Some(line) = default_line {
            assert!(!line.contains("third-party"), "third-party must not be a default feature: {line}");
        }
    }

    #[test]
    fn third_party_feature_exists_so_the_guard_means_something() {
        let manifest = include_str!("../Cargo.toml");
        assert!(manifest.contains("third-party = []"));
    }
}
