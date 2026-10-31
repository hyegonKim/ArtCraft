//! First-party domain names, validated at compile time.
//!
//! ```
//! use domains::artcraft::ARTCRAFT_DESKTOP;
//!
//! assert_eq!(ARTCRAFT_DESKTOP.https_link_without_path(), "https://desktop.getartcraft.com");
//! ```
//!
//! Enable the `url` feature for `Domain::https_url()`, a cached `&'static url::Url`.

pub mod artcraft;
pub mod fakeyou;
pub mod storyteller;

mod domain;
mod validate;

#[cfg(feature = "url")]
mod url_cache;

pub use domain::Domain;
