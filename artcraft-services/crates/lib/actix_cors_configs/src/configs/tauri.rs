use actix_cors::Cors;

pub fn add_tauri(cors: Cors, _is_production: bool) -> Cors {
  cors
      // Tauri Windows
      .allowed_origin("http://tauri.localhost")
      // Tauri Mac
      // DO NOT REMOVE: shipped ArtCraft desktop builds send this origin.
      // See deployed_desktop_client_origin_tests.rs.
      .allowed_origin("tauri://localhost")
}

#[cfg(test)]
mod tests {
  use actix_web::dev::Transform;
  use actix_web::http::{header, Method, StatusCode};
  use actix_web::test::{self, TestRequest};
  use server_environment::ServerEnvironment;

  use crate::cors::build_cors_config;
  use crate::testing::assert_origin_invalid;

  const WINDOWS_ORIGIN: &str = "http://tauri.localhost";
  const MAC_ORIGIN: &str = "tauri://localhost";

  mod windows {
    use super::*;

    #[actix_rt::test]
    async fn requests_and_preflights_allow_credentials() {
      assert_requests_and_preflights_allow_credentials(WINDOWS_ORIGIN).await;
    }

    #[actix_rt::test]
    async fn production_rejects_origin_variants() {
      let cors = build_cors_config(ServerEnvironment::Production);
      for origin in [
        "https://tauri.localhost",
        "http://tauri.localhost:8080",
        "http://tauri.localhost.example.com",
        "http://sub.tauri.localhost",
      ] {
        assert_origin_invalid(&cors, origin).await;
      }
    }
  }

  mod mac {
    use super::*;

    #[actix_rt::test]
    async fn requests_and_preflights_allow_credentials() {
      assert_requests_and_preflights_allow_credentials(MAC_ORIGIN).await;
    }

    #[actix_rt::test]
    async fn production_rejects_origin_variants() {
      let cors = build_cors_config(ServerEnvironment::Production);
      for origin in [
        "http://localhost",
        "https://localhost",
        "tauri://localhost:8080",
        "tauri://localhost.example.com",
        "tauri://sub.localhost",
      ] {
        assert_origin_invalid(&cors, origin).await;
      }
    }
  }

  async fn assert_requests_and_preflights_allow_credentials(origin: &str) {
    for environment in [ServerEnvironment::Production, ServerEnvironment::Development] {
      let service = build_cors_config(environment)
          .new_transform(test::ok_service())
          .await
          .unwrap();

      for method in [Method::GET, Method::POST, Method::OPTIONS] {
        let mut request = TestRequest::default()
            .uri("/v1/session")
            .insert_header((header::ORIGIN, origin));
        if method == Method::OPTIONS {
          request = request
              .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "POST"))
              .insert_header((header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type,session,x-artcraft-version"));
        }
        let response = test::call_service(&service, request.method(method).to_srv_request()).await;
        assert_eq!(response.status(), StatusCode::OK, "Origin: {}", origin);
        assert_eq!(response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(), origin);
        assert_eq!(response.headers().get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).unwrap(), "true");
      }
    }
  }
}
