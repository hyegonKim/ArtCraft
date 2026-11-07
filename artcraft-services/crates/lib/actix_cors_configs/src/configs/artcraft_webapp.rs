use actix_cors::Cors;
use crate::util::netlify_branch_domain_matches::netlify_branch_domain_matches;
use domains::artcraft::ARTCRAFT_WEBAPP;
use std::env;
use url::Url;

pub fn add_artcraft_webapp(cors: Cors, _is_production: bool) -> Cors {
  let cors = cors
      // Actual domains
      .allowed_origin(ARTCRAFT_WEBAPP.https_link_without_path())
      // Netlify project
      .allowed_origin_fn(|origin, _req_head| {
        netlify_branch_domain_matches(origin, "artcraft-webapp.netlify.app")
      });

  match env::var("ARTCRAFT_WEBAPP_EXTRA_ORIGIN") {
    Ok(origin) => add_extra_https_origin(cors, &origin),
    Err(_) => cors,
  }
}

fn add_extra_https_origin(cors: Cors, origin: &str) -> Cors {
  if let Ok(url) = Url::parse(origin) {
    if url.scheme() == "https" && url.origin().ascii_serialization() == origin {
      return cors.allowed_origin(origin);
    }
  }
  log::warn!("Ignoring ARTCRAFT_WEBAPP_EXTRA_ORIGIN: expected a single HTTPS origin");
  cors
}

#[cfg(test)]
mod tests {
  use crate::testing::{assert_exact_https_origin, assert_netlify_project, isolated_cors};

  use super::{add_artcraft_webapp, add_extra_https_origin};

  #[actix_rt::test]
  async fn optional_origin_is_exact() {
    const ORIGIN: &str = "https://artcraft-web-private-staging.netlify.app";
    let cors = isolated_cors(|cors, _| add_extra_https_origin(cors, ORIGIN), true);
    assert_exact_https_origin(&cors, ORIGIN).await;
  }

  #[actix_rt::test]
  async fn webapp_domain() {
    for is_production in [true, false] {
      let cors = isolated_cors(add_artcraft_webapp, is_production);
      assert_exact_https_origin(&cors, "https://app.getartcraft.com").await;
    }
  }

  #[actix_rt::test]
  async fn netlify_project() {
    let cors = isolated_cors(add_artcraft_webapp, true);
    assert_netlify_project(&cors, "artcraft-webapp.netlify.app").await;
  }
}
