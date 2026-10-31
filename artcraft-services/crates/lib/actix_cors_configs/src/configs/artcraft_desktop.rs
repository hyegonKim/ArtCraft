use actix_cors::Cors;
use domains::artcraft::ARTCRAFT_DESKTOP;

pub fn add_artcraft_desktop(cors: Cors, _is_production: bool) -> Cors {
  cors
      // Stable identity sent by the Artcraft native HTTP bridge (dev and packaged apps).
      .allowed_origin(ARTCRAFT_DESKTOP.https_link_without_path())
}

#[cfg(test)]
mod tests {
  use actix_web::dev::Transform;
  use actix_web::http::{header, Method, StatusCode};
  use actix_web::test::{self, TestRequest};
  use server_environment::ServerEnvironment;

  use crate::cors::build_cors_config;
  use crate::testing::{assert_origin_invalid, assert_origin_ok};

  const DESKTOP_ORIGIN: &str = "https://desktop.getartcraft.com";

  #[actix_rt::test]
  async fn desktop_requests_and_preflights_allow_credentials() {
    for environment in [ServerEnvironment::Production, ServerEnvironment::Development] {
      let service = build_cors_config(environment)
          .new_transform(test::ok_service())
          .await
          .unwrap();

      for method in [Method::GET, Method::POST, Method::OPTIONS] {
        let mut request = TestRequest::default()
            .uri("/v1/session")
            .insert_header((header::ORIGIN, DESKTOP_ORIGIN));
        if method == Method::OPTIONS {
          request = request
              .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "POST"))
              .insert_header((header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type,session,x-artcraft-version"));
        }
        let response = test::call_service(&service, request.method(method).to_srv_request()).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(), DESKTOP_ORIGIN);
        assert_eq!(response.headers().get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).unwrap(), "true");
      }
    }
  }

  #[actix_rt::test]
  async fn desktop_origin_is_an_exact_https_allowlist_entry() {
    let cors = build_cors_config(ServerEnvironment::Production);
    for origin in [
      "http://desktop.getartcraft.com",
      "https://desktop.getartcraft.com:8443",
      "https://desktop.getartcraft.com.example.com",
      "https://sub.desktop.getartcraft.com",
    ] {
      assert_origin_invalid(&cors, origin).await;
    }
  }

  #[actix_rt::test]
  async fn existing_desktop_clients_remain_allowed() {
    let cors = build_cors_config(ServerEnvironment::Production);
    for origin in ["https://studio.storyteller.ai", "http://tauri.localhost", "tauri://localhost"] {
      assert_origin_ok(&cors, origin).await;
    }
  }
}
