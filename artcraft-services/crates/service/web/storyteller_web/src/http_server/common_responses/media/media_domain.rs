use domains::fakeyou::FAKEYOU_CDN;
use url::Url;

/// Which domain to generate CDN, etc. links for.
#[derive(Copy, Clone, Debug)]
pub enum MediaDomain {
  FakeYou,
  Storyteller,
}

impl MediaDomain {
  pub fn new_cdn_url(&self) -> Url {
    match self {
      // NB: Storyteller media is served from the FakeYou CDN too.
      MediaDomain::FakeYou | MediaDomain::Storyteller => FAKEYOU_CDN.https_url().clone(),
    }
  }
  pub fn cdn_url_str(&self) -> &'static str {
    match self {
      MediaDomain::FakeYou | MediaDomain::Storyteller => FAKEYOU_CDN.https_link_without_path(),
    }
  }
}
