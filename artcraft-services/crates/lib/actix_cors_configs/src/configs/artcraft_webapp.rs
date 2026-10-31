use actix_cors::Cors;
use crate::util::netlify_branch_domain_matches::netlify_branch_domain_matches;
use domains::artcraft::ARTCRAFT_WEBAPP;

pub fn add_artcraft_webapp(cors: Cors, _is_production: bool) -> Cors {
  cors
      // Actual domains
      .allowed_origin(ARTCRAFT_WEBAPP.https_link_without_path())
      // Netlify project
      .allowed_origin_fn(|origin, _req_head| {
        netlify_branch_domain_matches(origin, "artcraft-webapp.netlify.app")
      })
}

#[cfg(test)]
mod tests {
  use crate::testing::{assert_exact_https_origin, assert_netlify_project, isolated_cors};

  use super::add_artcraft_webapp;

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
