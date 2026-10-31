//! Storyteller domains. Storyteller media is served from `fakeyou::FAKEYOU_CDN`.

use crate::{Domain, domain};

/// Main website.
pub const STORYTELLER: Domain = domain!("storyteller.ai");

/// HTTP API.
pub const STORYTELLER_API: Domain = domain!("api.storyteller.ai");

pub const STORYTELLER_ENGINE: Domain = domain!("engine.storyteller.ai");
pub const STORYTELLER_STAGING: Domain = domain!("staging.storyteller.ai");

/// Storyteller Studio and its deploy environments.
///
/// NB: Shipped ArtCraft desktop builds send `https://studio.storyteller.ai` as
/// their CORS origin, so it must stay allowed (see `actix_cors_configs`).
pub const STORYTELLER_STUDIO: Domain = domain!("studio.storyteller.ai");
pub const STORYTELLER_STUDIO_STAGING: Domain = domain!("studio-staging.studio.storyteller.ai");
pub const STORYTELLER_STUDIO_TESTING: Domain = domain!("studio-testing.studio.storyteller.ai");

/// Gen2 Studio.
pub const STORYTELLER_ANIMATE: Domain = domain!("animate.storyteller.ai");

pub const STORYTELLER_BOARD: Domain = domain!("board.storyteller.ai");
pub const STORYTELLER_RENDER: Domain = domain!("render.storyteller.ai");

/// Legacy hosts for the ArtCraft 2D and 3D apps.
pub const STORYTELLER_2D: Domain = domain!("2d.storyteller.ai");
pub const STORYTELLER_3D: Domain = domain!("3d.storyteller.ai");

/// Local development hosts (resolve to a developer machine).
pub const STORYTELLER_DEV: Domain = domain!("dev.storyteller.ai");
pub const STORYTELLER_DEV_PROXY: Domain = domain!("devproxy.storyteller.ai");

/// Storyteller Studio static assets.
pub const STORYTELLER_CDN: Domain = domain!("cdn.storyteller.ai");

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn website() {
    assert_eq!(STORYTELLER.bare_domain(), "storyteller.ai");
    assert_eq!(STORYTELLER.https_link_without_path(), "https://storyteller.ai");
    assert_eq!(STORYTELLER.https_link_with_root_path(), "https://storyteller.ai/");
  }

  #[test]
  fn api() {
    assert_eq!(STORYTELLER_API.https_link_without_path(), "https://api.storyteller.ai");
  }

  #[test]
  fn studio() {
    assert_eq!(STORYTELLER_STUDIO.https_link_without_path(), "https://studio.storyteller.ai");
    assert_eq!(STORYTELLER_STUDIO_STAGING.https_link_without_path(), "https://studio-staging.studio.storyteller.ai");
    assert_eq!(STORYTELLER_STUDIO_TESTING.https_link_without_path(), "https://studio-testing.studio.storyteller.ai");
  }

  #[test]
  fn apps() {
    assert_eq!(STORYTELLER_BOARD.https_link_without_path(), "https://board.storyteller.ai");
    assert_eq!(STORYTELLER_RENDER.https_link_without_path(), "https://render.storyteller.ai");
    assert_eq!(STORYTELLER_2D.https_link_without_path(), "https://2d.storyteller.ai");
    assert_eq!(STORYTELLER_3D.https_link_without_path(), "https://3d.storyteller.ai");
  }

  #[test]
  fn development() {
    assert_eq!(STORYTELLER_DEV.bare_domain(), "dev.storyteller.ai");
    assert_eq!(STORYTELLER_DEV_PROXY.bare_domain(), "devproxy.storyteller.ai");
  }

  #[test]
  fn cdn() {
    assert_eq!(STORYTELLER_CDN.https_link_without_path(), "https://cdn.storyteller.ai");
  }
}
