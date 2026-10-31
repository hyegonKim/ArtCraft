/// Everything about a response that helps decide whether Cloudflare's edge
/// produced it. Build with [`Self::new`] and add whichever headers the HTTP
/// client exposes; the body-only path works without any.
#[derive(Debug, Clone, Default)]
pub struct CloudflareResponseSignals<'a> {
  pub status_code: u16,

  pub body: &'a str,

  /// The `server` response header. Cloudflare's edge sets `cloudflare`.
  pub maybe_server_header: Option<&'a str>,

  /// The `cf-ray` header: present on every response that passed through
  /// Cloudflare (origin responses included).
  pub maybe_cf_ray: Option<&'a str>,

  /// The `cf-mitigated` header: `challenge` when the response IS a
  /// bot-management challenge. The definitive signal when available.
  pub maybe_cf_mitigated: Option<&'a str>,
}

impl<'a> CloudflareResponseSignals<'a> {
  pub fn new(status_code: u16, body: &'a str) -> Self {
    Self {
      status_code,
      body,
      maybe_server_header: None,
      maybe_cf_ray: None,
      maybe_cf_mitigated: None,
    }
  }

  pub fn with_server_header(mut self, server: Option<&'a str>) -> Self {
    self.maybe_server_header = server;
    self
  }

  pub fn with_cf_ray(mut self, cf_ray: Option<&'a str>) -> Self {
    self.maybe_cf_ray = cf_ray;
    self
  }

  pub fn with_cf_mitigated(mut self, cf_mitigated: Option<&'a str>) -> Self {
    self.maybe_cf_mitigated = cf_mitigated;
    self
  }

  /// Whether the headers prove the response passed through Cloudflare.
  /// `None` means we weren't given headers, not that it didn't.
  pub fn headers_say_cloudflare(&self) -> Option<bool> {
    if self.maybe_cf_ray.is_some() || self.maybe_cf_mitigated.is_some() {
      return Some(true);
    }
    self.maybe_server_header.map(|server| server.eq_ignore_ascii_case("cloudflare"))
  }
}
