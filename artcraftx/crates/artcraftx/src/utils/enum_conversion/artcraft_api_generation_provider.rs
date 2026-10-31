//! The `generation_provider` value the ArtCraft (storyteller-web) API accepts
//! when we upload results or create prompts.
//!
//! The API's `GenerationProvider` enum (storyteller-web,
//! `crates/schema/public/enums/src/common/generation_provider.rs`) accepts:
//! `artcraft`, `fal`, `grok`, `midjourney`, `sora`, `world_labs`,
//! `higgsfield`, `krea`, `leonardo`, `magnific`, `openart`, `picsart`,
//! `pixverse`, `runway`. Sending anything else fails the request with
//! `unknown variant`, so only these are ever sent; credential services
//! collapse onto their provider, and sources with no API value go
//! "unspecified" rather than losing the upload / prompt.
//!
//! NB: the app only records tasks for providers it generates with, so the
//! cookie-only services (Magnific, OpenArt, Runway) and the API-key-only
//! ones (OpenAI, Replicate) have no bare [`GenerationSource`] variant to
//! send yet; add one alongside their generation path.

use core_types::enums::generation_source::GenerationSource;

pub fn artcraft_api_generation_provider(source: GenerationSource) -> Option<GenerationSource> {
  match source {
    GenerationSource::Artcraft
    | GenerationSource::ArtcraftLocal
    | GenerationSource::ArtcraftCookies
    | GenerationSource::ArtcraftApi => Some(GenerationSource::Artcraft),
    GenerationSource::Fal
    | GenerationSource::FalApi => Some(GenerationSource::Fal),
    GenerationSource::Grok
    | GenerationSource::GrokCookies
    | GenerationSource::XAiApi => Some(GenerationSource::Grok),
    GenerationSource::Midjourney
    | GenerationSource::MidjourneyCookies => Some(GenerationSource::Midjourney),
    GenerationSource::WorldLabs
    | GenerationSource::WorldLabsCookies => Some(GenerationSource::WorldLabs),
    GenerationSource::Higgsfield
    | GenerationSource::HiggsfieldCookies => Some(GenerationSource::Higgsfield),
    // The API knows `magnific` / `openart` / `runway`, but the app has no bare
    // provider variant for them yet (see the module docs).
    GenerationSource::MagnificCookies
    | GenerationSource::OpenArtCookies
    | GenerationSource::RunwayCookies
    | GenerationSource::OpenAiApi
    | GenerationSource::ReplicateApi => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// What storyteller-web's `GenerationProvider` deserializes today.
  const API_KNOWN_VALUES: &[&str] = &[
    "artcraft", "fal", "grok", "midjourney", "sora", "world_labs",
    "higgsfield", "krea", "leonardo", "magnific", "openart", "picsart", "pixverse", "runway",
  ];

  #[test]
  fn only_values_the_api_knows_are_ever_sent() {
    for source in GenerationSource::all_variants() {
      if let Some(sent) = artcraft_api_generation_provider(source) {
        assert!(API_KNOWN_VALUES.contains(&sent.to_str()), "{source} would be sent as {sent}, which the API rejects");
      }
    }
  }

  #[test]
  fn credential_services_collapse_onto_their_provider() {
    assert_eq!(artcraft_api_generation_provider(GenerationSource::GrokCookies), Some(GenerationSource::Grok));
    assert_eq!(artcraft_api_generation_provider(GenerationSource::ArtcraftLocal), Some(GenerationSource::Artcraft));
    assert_eq!(artcraft_api_generation_provider(GenerationSource::WorldLabs), Some(GenerationSource::WorldLabs));
  }

  #[test]
  fn higgsfield_is_sent_as_itself() {
    assert_eq!(artcraft_api_generation_provider(GenerationSource::Higgsfield), Some(GenerationSource::Higgsfield));
    assert_eq!(artcraft_api_generation_provider(GenerationSource::HiggsfieldCookies), Some(GenerationSource::Higgsfield));
    assert_eq!(GenerationSource::Higgsfield.to_str(), "higgsfield");
  }

  #[test]
  fn services_without_a_bare_provider_stay_unspecified() {
    for source in [GenerationSource::MagnificCookies, GenerationSource::OpenArtCookies, GenerationSource::RunwayCookies, GenerationSource::OpenAiApi, GenerationSource::ReplicateApi] {
      assert_eq!(artcraft_api_generation_provider(source), None, "{source}");
    }
  }
}
