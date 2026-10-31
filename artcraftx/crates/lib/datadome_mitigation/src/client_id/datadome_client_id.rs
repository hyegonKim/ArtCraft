/// The cookie DataDome sets once it has profiled a client. Its value is the
/// client id.
pub const DATADOME_COOKIE: &str = "datadome";

/// The header DataDome's JS adds to API requests. Its value is the
/// `datadome` cookie's value — a request sending the cookie without the
/// header (or with a mismatched one) looks like a script, not the browser
/// that earned the cookie.
pub const DATADOME_CLIENT_ID_HEADER: &str = "x-datadome-clientid";

/// The DataDome client id carried by a cookie header, if any.
pub fn client_id_from_cookie_header(cookie_header: &str) -> Option<String> {
  cookie_header
      .split(';')
      .filter_map(|pair| pair.trim().split_once('='))
      .find(|(name, _)| name.trim() == DATADOME_COOKIE)
      .map(|(_, value)| value.trim().to_string())
      .filter(|value| !value.is_empty())
}

/// The `(name, value)` header a replaying client should add, derived from
/// the cookie header. `None` when there's no `datadome` cookie — then the
/// site hasn't profiled this session and the header would be a lie.
pub fn client_id_header_for_cookie_header(cookie_header: &str) -> Option<(&'static str, String)> {
  client_id_from_cookie_header(cookie_header).map(|value| (DATADOME_CLIENT_ID_HEADER, value))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extracts_client_id() {
    let header = "__client=abc; datadome=Yw0QRSl16BFXimBAv1I7~SfJW6aNcwFV; _ga=1";
    assert_eq!(client_id_from_cookie_header(header).as_deref(), Some("Yw0QRSl16BFXimBAv1I7~SfJW6aNcwFV"));
    assert_eq!(
      client_id_header_for_cookie_header(header),
      Some(("x-datadome-clientid", "Yw0QRSl16BFXimBAv1I7~SfJW6aNcwFV".to_string())),
    );
  }

  #[test]
  fn absent_or_empty_cookie_yields_none() {
    assert_eq!(client_id_from_cookie_header("a=1; b=2"), None);
    assert_eq!(client_id_from_cookie_header("datadome="), None);
    assert_eq!(client_id_header_for_cookie_header(""), None);
  }
}
