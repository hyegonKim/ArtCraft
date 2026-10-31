use std::net::Ipv4Addr;

use actix_cors::Cors;
use log::warn;
use url::{Host, Url};

pub fn add_development_only(cors: Cors) -> Cors {
  // Any localhost is allowed.
  cors.allowed_origin_fn(|origin, _req_head| {
        let maybe_url = origin.to_str()
            .map(|origin| Url::parse(origin));

        let url = match maybe_url {
          Ok(Ok(url)) => url,
          _ => {
            warn!("Invalid origin: {:?}", origin);
            return false
          },
        };

        match url.host() {
          Some(Host::Domain("localhost")) => true,
          Some(Host::Ipv4(Ipv4Addr::LOCALHOST)) => true,
          _ => false,
        }
      })
}

#[cfg(test)]
mod tests {
  use actix_cors::Cors;

  use crate::testing::{assert_origin_invalid, assert_origin_ok};

  use super::add_development_only;

  #[actix_rt::test]
  async fn any_localhost_is_allowed() {
    let cors = development_only_cors();
    for origin in [
      "http://localhost",
      "http://localhost:3000",
      "https://localhost:8443",
      "http://127.0.0.1",
      "http://127.0.0.1:5173",
    ] {
      assert_origin_ok(&cors, origin).await;
    }
  }

  #[actix_rt::test]
  async fn non_localhost_is_rejected() {
    let cors = development_only_cors();
    for origin in [
      "https://getartcraft.com",
      "http://localhost.evil.example",
      "http://evil.localhost.example",
      "http://127.0.0.2",
      "http://192.168.1.10",
      "http://[::1]", // NB: IPv6 loopback isn't allowed today
      "not a url",
      "null",
    ] {
      assert_origin_invalid(&cors, origin).await;
    }
  }

  fn development_only_cors() -> Cors {
    add_development_only(Cors::default())
        .block_on_origin_mismatch(true)
  }
}
