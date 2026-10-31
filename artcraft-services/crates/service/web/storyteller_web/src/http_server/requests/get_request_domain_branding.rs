use actix_helpers::extractors::get_request_origin_uri::get_request_origin_uri;
use actix_web::http::header::HOST;
use actix_web::HttpRequest;
use log::{debug, warn};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DomainBranding {
  ArtCraftDotAi,
  GetArtCraft,
  FakeYou,
  Storyteller,
}

pub fn get_request_domain_branding(http_request: &HttpRequest) -> Option<DomainBranding> {
  // NB: "Origin" vs "Referrer"
  //
  // Basically:
  //  - "In order to preserve privacy, any browser request can decide to omit the Referer header."
  //  - "The Origin header is similar to the Referer header, but does not disclose the path, and may be null."
  //  - "Origin" is sent on cross-origin, same-origin (except GET and HEAD requests - typically).
  //
  // Reading:
  //  - https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Origin
  //  - https://stackoverflow.com/a/71040145
  //
  let maybe_origin = get_request_origin_uri(&http_request);

  match maybe_origin {
    Ok(Some(uri)) => {
      if let Some(branding) = match_possible_hostname(uri.host()) {
        debug!("Origin header: {:?} Branding for hostname: {:?}", uri, branding);
        return Some(branding);
      }
    }
    // Fail open for now.
    Ok(None) => {}
    Err(err) => {
      warn!("Origin header error: {:?}", err);
    }
  }

  // NB: The "HOST" header contains the hostname. The `http_request.uri()` method we tried
  // before does not include the hostname at all - it only includes the path (!), which is
  // why we use "HOST" header instead.
  let maybe_host_header = http_request.headers()
      .get(HOST)
      .map(|header| header.to_str().ok())
      .flatten();

  if let Some(branding) = match_possible_hostname(maybe_host_header) {
    debug!("Host header: {:?} Branding for hostname: {:?}", maybe_host_header, branding);
    return Some(branding);
  }

  None
}

fn match_possible_hostname(maybe_hostname: Option<&str>) -> Option<DomainBranding> {
  let hostname = match maybe_hostname {
    Some(hostname) => hostname,
    None => return None,
  };
  match hostname {
    host if host.contains("artcraft.ai") => Some(DomainBranding::ArtCraftDotAi),
    host if host.contains("getartcraft") => Some(DomainBranding::GetArtCraft),
    host if host.contains("fakeyou") => Some(DomainBranding::FakeYou),
    host if host.contains("storyteller") => Some(DomainBranding::Storyteller),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use actix_web::test::TestRequest;

  use crate::http_server::requests::get_request_domain_branding::{get_request_domain_branding, DomainBranding};

  mod origin_header {
    use super::*;
    
    #[test]
    fn getartcraft_dot_com() {
      let request = TestRequest::get()
          .insert_header(("origin", "https://getartcraft.com"))
          .to_http_request();
      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::GetArtCraft));
    }
    
    #[test]
    fn api_dot_getartcraft_dot_com() {
      let request = TestRequest::get()
          .insert_header(("origin", "https://api.getartcraft.com"))
          .to_http_request();
      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::GetArtCraft));
    }


    #[test]
    fn artcraft_dot_ai() {
      let request = TestRequest::get()
          .insert_header(("origin", "https://artcraft.ai"))
          .to_http_request();
      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::ArtCraftDotAi));
    }

    #[test]
    fn api_dot_artcraft_dot_ai() {
      let request = TestRequest::get()
          .insert_header(("origin", "https://api.artcraft.ai"))
          .to_http_request();
      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::ArtCraftDotAi));
    }

    #[test]
    fn fakeyou_dot_com() {
      let request = TestRequest::get()
          .insert_header(("origin", "https://fakeyou.com"))
          .to_http_request();
      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::FakeYou));
    }

    #[test]
    fn api_dot_fakeyou_dot_com() {
      let request = TestRequest::get()
          .insert_header(("origin", "https://api.fakeyou.com"))
          .to_http_request();
      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::FakeYou));
    }

    #[test]
    fn storyteller_dot_ai() {
      let request = TestRequest::get()
          .insert_header(("origin", "https://storyteller.ai"))
          .to_http_request();
      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::Storyteller));
    }

    #[test]
    fn api_dot_storyteller_dot_ai() {
      let request = TestRequest::get()
          .insert_header(("origin", "https://api.storyteller.ai"))
          .to_http_request();
      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::Storyteller));
    }
  }

  mod host_header {
    use super::*;

    fn getartcraft_dot_com() {
      let request = TestRequest::get()
          .insert_header(("host", "getartcraft.com"))
          .to_http_request();
      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::GetArtCraft));
    }

    #[test]
    fn api_dot_getartcraft_dot_com() {
      let request = TestRequest::get()
          .insert_header(("host", "api.getartcraft.com"))
          .to_http_request();
      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::GetArtCraft));
    }
    
    #[test]
    fn fakeyou_dot_com() {
      let request = TestRequest::get()
          .insert_header(("host", "fakeyou.com"))
          .to_http_request();
      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::FakeYou));
    }

    #[test]
    fn api_dot_fakeyou_dot_com() {
      let request = TestRequest::get()
          .insert_header(("host", "api.fakeyou.com"))
          .to_http_request();
      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::FakeYou));
    }

    #[test]
    fn storyteller_dot_ai() {
      let request = TestRequest::get()
          .insert_header(("host", "storyteller.ai"))
          .to_http_request();
      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::Storyteller));
    }

    #[test]
    fn api_dot_storyteller_dot_ai() {
      let request = TestRequest::get()
          .insert_header(("host", "api.storyteller.ai"))
          .to_http_request();
      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::Storyteller));
    }
  }

//  mod uri {
//    use super::*;
//
//    #[test]
//    fn fakeyou_dot_com() {
//      let request = TestRequest::get()
//          .uri("https://fakeyou.com")
//          .to_http_request();
//      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::FakeYou));
//    }
//
//    #[test]
//    fn api_dot_fakeyou_dot_com() {
//      let request = TestRequest::get()
//          .uri("https://api.fakeyou.com")
//          .to_http_request();
//      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::FakeYou));
//    }
//
//    #[test]
//    fn storyteller_dot_ai() {
//      let request = TestRequest::get()
//          .uri("https://storyteller.ai")
//          .to_http_request();
//      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::Storyteller));
//    }
//
//    #[test]
//    fn api_dot_storyteller_dot_ai() {
//      let request = TestRequest::get()
//          .uri("https://api.storyteller.ai")
//          .to_http_request();
//      assert_eq!(get_request_domain_branding(&request), Some(DomainBranding::Storyteller));
//    }
//  }
}
