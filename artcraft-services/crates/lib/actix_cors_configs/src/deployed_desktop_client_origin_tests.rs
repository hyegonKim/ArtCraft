//! DO NOT REMOVE THESE ORIGINS: shipped ArtCraft desktop builds depend on them.
//!
//! Old ArtCraft desktop builds in the wild send one of these `Origin` headers:
//!
//!   * `https://studio.storyteller.ai`
//!   * `tauri://localhost` (macOS Tauri webview)
//!
//! Those builds can't be updated server-side. If the API stops allowing these
//! origins, those installs break: every credentialed request (login, session,
//! generation) fails CORS in the webview.
//!
//! These tests live in their own module on purpose, apart from the Storyteller
//! and Tauri configs, so reworking or deleting those configs (or their tests)
//! can't silently remove this protection. They use string literals, not
//! `domains` constants, so renaming a constant can't change what they check.

use actix_web::dev::Transform;
use actix_web::http::{header, Method, StatusCode};
use actix_web::test::{self, TestRequest};
use server_environment::ServerEnvironment;

use crate::cors::build_cors_config;

const STORYTELLER_STUDIO_ORIGIN: &str = "https://studio.storyteller.ai";
const TAURI_MAC_ORIGIN: &str = "tauri://localhost";

const ENVIRONMENTS: [ServerEnvironment; 2] = [ServerEnvironment::Production, ServerEnvironment::Development];

mod storyteller_studio_origin {
  use super::*;

  #[actix_rt::test]
  async fn credentialed_requests_are_allowed() {
    assert_credentialed_requests_allowed(STORYTELLER_STUDIO_ORIGIN).await;
  }

  #[actix_rt::test]
  async fn preflights_are_allowed() {
    assert_preflights_allowed(STORYTELLER_STUDIO_ORIGIN).await;
  }
}

mod tauri_mac_origin {
  use super::*;

  #[actix_rt::test]
  async fn credentialed_requests_are_allowed() {
    assert_credentialed_requests_allowed(TAURI_MAC_ORIGIN).await;
  }

  #[actix_rt::test]
  async fn preflights_are_allowed() {
    assert_preflights_allowed(TAURI_MAC_ORIGIN).await;
  }
}

/// Simple requests in every environment and method get the origin echoed
/// back with credentials allowed, which is what the webview requires.
async fn assert_credentialed_requests_allowed(origin: &str) {
  for environment in ENVIRONMENTS {
    let service = build_cors_config(environment)
        .new_transform(test::ok_service())
        .await
        .unwrap();

    for method in [Method::GET, Method::POST, Method::PUT, Method::DELETE] {
      let request = TestRequest::default()
          .method(method.clone())
          .uri("/v1/session")
          .insert_header((header::ORIGIN, origin))
          .to_srv_request();
      let response = test::call_service(&service, request).await;

      let context = format!(
        "Shipped ArtCraft desktop builds send Origin {} ({:?}, {}). DO NOT REMOVE this origin from the CORS config.",
        origin, environment, method);
      assert_eq!(response.status(), StatusCode::OK, "{}", context);
      assert_eq!(response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).map(|v| v.to_str().unwrap()), Some(origin), "{}", context);
      assert_eq!(response.headers().get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).map(|v| v.to_str().unwrap()), Some("true"), "{}", context);
    }
  }
}

/// Preflights for the methods and headers the desktop app uses are accepted.
async fn assert_preflights_allowed(origin: &str) {
  for environment in ENVIRONMENTS {
    let service = build_cors_config(environment)
        .new_transform(test::ok_service())
        .await
        .unwrap();

    for requested_method in ["GET", "POST", "PUT", "DELETE"] {
      let request = TestRequest::default()
          .method(Method::OPTIONS)
          .uri("/v1/session")
          .insert_header((header::ORIGIN, origin))
          .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, requested_method))
          .insert_header((header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type,session"))
          .to_srv_request();
      let response = test::call_service(&service, request).await;

      let context = format!(
        "Shipped ArtCraft desktop builds send Origin {} ({:?}, preflight {}). DO NOT REMOVE this origin from the CORS config.",
        origin, environment, requested_method);
      assert_eq!(response.status(), StatusCode::OK, "{}", context);
      assert_eq!(response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).map(|v| v.to_str().unwrap()), Some(origin), "{}", context);
      assert_eq!(response.headers().get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).map(|v| v.to_str().unwrap()), Some("true"), "{}", context);
    }
  }
}
