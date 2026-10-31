use actix_cors::Cors;
use crate::util::netlify_branch_domain_matches::netlify_branch_domain_matches;

/// CORS for the "Is That Real Seedance?" website (video-info-website).
pub fn add_realseedance(cors: Cors, _is_production: bool) -> Cors {
  cors
      // Actual domain
      .allowed_origin("https://realseedance.com")
      .allowed_origin("https://www.realseedance.com")
      // Netlify project (bare domain + branch / deploy-preview subdomains)
      .allowed_origin_fn(|origin, _req_head| {
        netlify_branch_domain_matches(origin, "real-seedance.netlify.app")
      })
}

#[cfg(test)]
mod tests {
  use crate::testing::{assert_exact_https_origin, assert_netlify_project, isolated_cors};

  use super::add_realseedance;

  #[actix_rt::test]
  async fn domains() {
    for is_production in [true, false] {
      let cors = isolated_cors(add_realseedance, is_production);
      assert_exact_https_origin(&cors, "https://realseedance.com").await;
      assert_exact_https_origin(&cors, "https://www.realseedance.com").await;
    }
  }

  #[actix_rt::test]
  async fn netlify_project() {
    let cors = isolated_cors(add_realseedance, true);
    assert_netlify_project(&cors, "real-seedance.netlify.app").await;
  }
}
