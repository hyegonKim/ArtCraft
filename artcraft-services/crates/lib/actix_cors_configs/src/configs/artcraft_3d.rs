use actix_cors::Cors;
use crate::util::netlify_branch_domain_matches::netlify_branch_domain_matches;
use domains::artcraft::ARTCRAFT_3D;
use domains::storyteller::STORYTELLER_3D;

pub fn add_artcraft_3d(cors: Cors, _is_production: bool) -> Cors {
  cors
      // Hypothetical domains
      .allowed_origin(STORYTELLER_3D.https_link_without_path())
      .allowed_origin(ARTCRAFT_3D.https_link_without_path())
      // Netlify project
      .allowed_origin_fn(|origin, _req_head| {
        netlify_branch_domain_matches(origin, "storyteller-3d.netlify.app")
      })
      // Tauri localhost (3D engine, first three ports)
      .allowed_origin("http://localhost:5173")
      .allowed_origin("http://localhost:5174") // If already started
      .allowed_origin("http://localhost:5175") // If already started
      .allowed_origin("https://macaroni-1.tailce84f.ts.net") // TODO
      .allowed_origin("https://halide.tailce84f.ts.net") // TODO
      .allowed_origin("https://brandons-macbook-pro.taild62114.ts.net") // TODO
}

#[cfg(test)]
mod tests {
  use crate::testing::{assert_exact_https_origin, assert_netlify_project, assert_origin_invalid, assert_origin_ok, isolated_cors};

  use super::add_artcraft_3d;

  #[actix_rt::test]
  async fn domains() {
    for is_production in [true, false] {
      let cors = isolated_cors(add_artcraft_3d, is_production);
      assert_exact_https_origin(&cors, "https://3d.getartcraft.com").await;
      assert_exact_https_origin(&cors, "https://3d.storyteller.ai").await;
    }
  }

  #[actix_rt::test]
  async fn tauri_localhost_ports() {
    let cors = isolated_cors(add_artcraft_3d, true);
    for origin in ["http://localhost:5173", "http://localhost:5174", "http://localhost:5175"] {
      assert_origin_ok(&cors, origin).await;
    }
    assert_origin_invalid(&cors, "http://localhost:5176").await;
    assert_origin_invalid(&cors, "http://localhost:5741").await; // The 2D engine's port
  }

  #[actix_rt::test]
  async fn tailscale_hosts() {
    let cors = isolated_cors(add_artcraft_3d, true);
    for origin in [
      "https://macaroni-1.tailce84f.ts.net",
      "https://halide.tailce84f.ts.net",
      "https://brandons-macbook-pro.taild62114.ts.net",
    ] {
      assert_exact_https_origin(&cors, origin).await;
    }
    assert_origin_invalid(&cors, "https://other-machine.tailce84f.ts.net").await;
  }

  #[actix_rt::test]
  async fn netlify_project() {
    let cors = isolated_cors(add_artcraft_3d, true);
    assert_netlify_project(&cors, "storyteller-3d.netlify.app").await;
  }
}
