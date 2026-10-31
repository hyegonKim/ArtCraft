use actix_cors::Cors;
use crate::util::netlify_branch_domain_matches::netlify_branch_domain_matches;
use domains::artcraft::ARTCRAFT_AI;
use domains::artcraft::ARTCRAFT_AI_WWW;
use domains::artcraft::ARTCRAFT_WEBSITE;
use domains::artcraft::ARTCRAFT_WEBSITE_WWW;

pub fn add_artcraft_website(cors: Cors, _is_production: bool) -> Cors {
  cors
      // Actual domains
      .allowed_origin(ARTCRAFT_WEBSITE.https_link_without_path())
      .allowed_origin(ARTCRAFT_WEBSITE_WWW.https_link_without_path())
      // Hypothetical domains
      .allowed_origin(ARTCRAFT_AI.https_link_without_path())
      .allowed_origin(ARTCRAFT_AI_WWW.https_link_without_path())
      // Development against production
      .allowed_origin("http://localhost:4200")
      .allowed_origin("http://localhost:4201")
      .allowed_origin("http://localhost:4202")
      // Netlify project
      .allowed_origin_fn(|origin, _req_head| {
        netlify_branch_domain_matches(origin, "artcraft-website.netlify.app")
      })
}

#[cfg(test)]
mod tests {
  use crate::testing::{assert_exact_https_origin, assert_netlify_project, assert_origin_invalid, assert_origin_ok, isolated_cors};

  use super::add_artcraft_website;

  #[actix_rt::test]
  async fn website_domains() {
    for is_production in [true, false] {
      let cors = isolated_cors(add_artcraft_website, is_production);
      for origin in [
        "https://getartcraft.com",
        "https://www.getartcraft.com",
        "https://artcraft.ai",
        "https://www.artcraft.ai",
      ] {
        assert_exact_https_origin(&cors, origin).await;
      }
    }
  }

  /// Local frontend builds talk to the production API.
  #[actix_rt::test]
  async fn localhost_development_against_production() {
    let cors = isolated_cors(add_artcraft_website, true);
    for origin in ["http://localhost:4200", "http://localhost:4201", "http://localhost:4202"] {
      assert_origin_ok(&cors, origin).await;
    }
    assert_origin_invalid(&cors, "http://localhost:4203").await;
    assert_origin_invalid(&cors, "https://localhost:4200").await;
  }

  #[actix_rt::test]
  async fn netlify_project() {
    let cors = isolated_cors(add_artcraft_website, true);
    assert_netlify_project(&cors, "artcraft-website.netlify.app").await;
  }
}
