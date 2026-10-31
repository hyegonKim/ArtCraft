use artcraft_client::utils::api_host::ApiHost;

use core_types::enums::generation_source::GenerationSource;

/// Port the local ArtCraft development server listens on.
pub const ARTCRAFT_LOCAL_DEV_PORT: u32 = 12345;

/// The API host an ArtCraft-family credential authenticates against.
/// `None` for services that aren't ArtCraft accounts.
pub fn maybe_artcraft_api_host_for_service(
  service: GenerationSource,
) -> Option<ApiHost> {
  match service {
    GenerationSource::Artcraft
    | GenerationSource::ArtcraftCookies => Some(ApiHost::Storyteller),
    GenerationSource::ArtcraftLocal => {
      Some(ApiHost::Localhost { port: ARTCRAFT_LOCAL_DEV_PORT })
    }
    _ => None,
  }
}
