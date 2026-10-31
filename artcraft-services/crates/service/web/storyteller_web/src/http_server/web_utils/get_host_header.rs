use actix_http::header::{HeaderMap, HeaderName};
use actix_web::HttpRequest;

use errors::AnyhowResult;

// NB: This is extracted because CLion is having incredible difficulty with these types.
pub fn get_host_header(http_request: &HttpRequest) -> AnyhowResult<Option<String>> {
  let host_header_name  = HeaderName::from_static("host");
  let header_map : &HeaderMap = http_request.headers();
  let maybe_hostname = header_map.get(host_header_name);

  let hostname = maybe_hostname.map(|s| s.to_str())
      .map(|inner| inner.map(|s| s.to_string()))
      .transpose()?;

  Ok(hostname)
}