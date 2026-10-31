use core_types::enums::generation_source::GenerationSource;
use reqwest::Url;

/// The canonical site origin that a cookie-kind service's cookies belong to.
///
/// Used to attribute a domain when cookies arrive as a bare `Cookie:` header
/// string (hand-entered credentials, session strings from our own backend)
/// rather than from a browser capture that knows each cookie's real domain.
/// Returns `None` for API-key services, which have no cookies.
pub fn cookie_origin_for_service(service: GenerationSource) -> Option<Url> {
  let origin = match service {
    GenerationSource::Artcraft
    | GenerationSource::ArtcraftLocal
    | GenerationSource::ArtcraftCookies => "https://storyteller.ai/",
    GenerationSource::Grok
    | GenerationSource::GrokCookies => "https://grok.com/",
    GenerationSource::Midjourney
    | GenerationSource::MidjourneyCookies => "https://www.midjourney.com/",
    GenerationSource::WorldLabs
    | GenerationSource::WorldLabsCookies => "https://worldlabs.ai/",
    GenerationSource::Higgsfield
    | GenerationSource::HiggsfieldCookies => "https://higgsfield.ai/",
    GenerationSource::MagnificCookies => "https://www.magnific.com/",
    GenerationSource::OpenArtCookies => "https://openart.ai/",
    GenerationSource::RunwayCookies => "https://app.runwayml.com/",
    GenerationSource::ArtcraftApi
    | GenerationSource::Fal
    | GenerationSource::FalApi
    | GenerationSource::OpenAiApi
    | GenerationSource::ReplicateApi
    | GenerationSource::XAiApi => return None,
  };
  Some(Url::parse(origin).expect("static origin URL should parse"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use core_types::enums::generation_source::CredentialKind;

  #[test]
  fn every_cookie_service_has_an_origin() {
    for service in GenerationSource::all_variants() {
      let origin = cookie_origin_for_service(service);
      match service.kind() {
        CredentialKind::Cookies => {
          assert!(origin.is_some(), "cookie service {service} has no cookie origin");
        }
        CredentialKind::ApiKey => {
          assert!(origin.is_none(), "api-key service {service} should have no cookie origin");
        }
      }
    }
  }
}
