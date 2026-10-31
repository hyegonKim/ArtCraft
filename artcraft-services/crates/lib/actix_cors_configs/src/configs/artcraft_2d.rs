use actix_cors::Cors;
use crate::util::netlify_branch_domain_matches::netlify_branch_domain_matches;
use domains::artcraft::ARTCRAFT_2D;
use domains::storyteller::STORYTELLER_2D;

pub fn add_artcraft_2d(cors: Cors, _is_production: bool) -> Cors {
  cors
      // Hypothetical domains
      .allowed_origin(STORYTELLER_2D.https_link_without_path())
      .allowed_origin(ARTCRAFT_2D.https_link_without_path())
      // Netlify project
      .allowed_origin_fn(|origin, _req_head| {
        netlify_branch_domain_matches(origin, "storyteller-2d.netlify.app")
      })
      // Tauri localhost (2D engine, first three ports)
      .allowed_origin("http://localhost:5741")
      .allowed_origin("http://localhost:5742") // If already started
      .allowed_origin("http://localhost:5743") // If already started
}

#[cfg(test)]
mod tests {
  use crate::testing::{assert_exact_https_origin, assert_netlify_project, assert_origin_invalid, assert_origin_ok, isolated_cors};

  use super::add_artcraft_2d;

  #[actix_rt::test]
  async fn domains() {
    for is_production in [true, false] {
      let cors = isolated_cors(add_artcraft_2d, is_production);
      assert_exact_https_origin(&cors, "https://2d.getartcraft.com").await;
      assert_exact_https_origin(&cors, "https://2d.storyteller.ai").await;
    }
  }

  #[actix_rt::test]
  async fn tauri_localhost_ports() {
    let cors = isolated_cors(add_artcraft_2d, true);
    for origin in ["http://localhost:5741", "http://localhost:5742", "http://localhost:5743"] {
      assert_origin_ok(&cors, origin).await;
    }
    assert_origin_invalid(&cors, "http://localhost:5744").await;
    assert_origin_invalid(&cors, "http://localhost:5173").await; // The 3D engine's port
  }

  #[actix_rt::test]
  async fn netlify_project() {
    let cors = isolated_cors(add_artcraft_2d, true);
    assert_netlify_project(&cors, "storyteller-2d.netlify.app").await;
  }
}
