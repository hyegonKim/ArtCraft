//! Real routes, signed sessions, and migrated local database fixtures.
//! The fixed-socket isolated harness cannot use production database URLs.
use std::sync::Arc;

use actix_http::Request;
use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::http::StatusCode;
use actix_web::{test, web, App};
use artcraft_api_defs::media_file::list::list_media_files_for_user_v2::ListMediaFilesForUserV2SuccessResponse;
use mysql_testing::fixtures::media_files::create_test_video_media_file;
use mysql_testing::fixtures::users::create_test_user;
use mysql_testing::isolated::IsolatedTestDatabase;
use serde_json::{json, Value};

use super::media_files_routes::add_media_file_routes;
use crate::http_server::endpoints::omni_gen::generate::video::tests::support::build_test_server_state;

const LEGACY_ITEM_FIELDS: &[&str] = &[
  "token",
  "media_class",
  "media_type",
  "maybe_engine_category",
  "maybe_animation_type",
  "origin",
  "origin_category",
  "origin_product_category",
  "maybe_origin_model_type",
  "maybe_origin_model_token",
  "public_bucket_path",
  "public_bucket_url",
  "media_links",
  "maybe_prompt_token",
  "cover_image",
  "creator_set_visibility",
  "is_user_upload",
  "is_intermediate_system_file",
  "maybe_title",
  "maybe_text_transcript",
  "maybe_style_name",
  "maybe_duration_millis",
  "stats",
  "created_at",
  "updated_at",
];

#[actix_web::test]
#[cfg_attr(feature = "skip_database_tests", ignore)]
async fn user_media_endpoint_versions_preserve_legacy_contract() {
  let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
  let db = IsolatedTestDatabase::create().await;
  let owner = create_test_user(&db.pool).await.unwrap();
  for _ in 0..26 {
    create_test_video_media_file(&db.pool, &owner.user_token, Some(1234))
      .await
      .unwrap();
  }
  let state = build_test_server_state(db.pool.clone(), "http://127.0.0.1:1".to_string());
  let cookie = state
    .session_cookie_manager
    .create_cookie(&owner.session_token, &owner.user_token)
    .unwrap()
    .into_owned();
  let app = test::init_service(add_media_file_routes(
    App::new().app_data(web::Data::new(Arc::new(state))),
  ))
  .await;
  let v1 = format!("/v1/media_files/list/user/{}", owner.username);
  let v2 = format!("/v1/media_files/list_v2/user/{}", owner.username);
  let session = Some(cookie.value());

  // Default upload exclusion, page size, and original JSON shape stay intact.
  let (status, excluded) = get(&app, &v1, session).await;
  assert_eq!(status, StatusCode::OK);
  assert_eq!(
    excluded,
    json!({"success": true, "results": [], "pagination": {"current": 0, "total_page_count": 1}})
  );
  let (status, legacy) = get(&app, &format!("{v1}?include_user_uploads=true"), session).await;
  assert_eq!(status, StatusCode::OK);
  assert_eq!(legacy.as_object().unwrap().len(), 3);
  assert_eq!(legacy["results"].as_array().unwrap().len(), 25);
  assert_eq!(legacy["pagination"], json!({"current": 0, "total_page_count": 2}));
  assert_legacy_item_keys(&legacy["results"][0]);

  // Exact multiples intentionally keep v1's historical extra page.
  let (_, exact) = get(&app, &format!("{v1}?include_user_uploads=true&page_size=26"), session).await;
  assert_eq!(exact["results"].as_array().unwrap().len(), 26);
  assert_eq!(exact["pagination"], json!({"current": 0, "total_page_count": 2}));
  // V2's size cap must never spill into the legacy endpoint.
  let (status, large) = get(&app, &format!("{v1}?include_user_uploads=true&page_size=101"), session).await;
  assert_eq!(status, StatusCode::OK);
  assert_eq!(large["results"].as_array().unwrap().len(), 26);
  assert_eq!(large["pagination"], json!({"current": 0, "total_page_count": 1}));
  let (_, last) = get(&app, &format!("{v1}?include_user_uploads=true&page_index=1"), session).await;
  assert_eq!(last["results"].as_array().unwrap().len(), 1);
  assert_eq!(last["pagination"], json!({"current": 1, "total_page_count": 2}));
  // The rejected count-mode option has no effect on the v1 contract.
  let (_, ignored_option) = get(
    &app,
    &format!("{v1}?include_user_uploads=true&include_total_count=false"),
    session,
  )
  .await;
  assert_eq!(ignored_option, legacy);

  let (status, first_v2) = get(&app, &format!("{v2}?include_user_uploads=true"), session).await;
  assert_eq!(status, StatusCode::OK);
  assert_eq!(first_v2.as_object().unwrap().len(), 3);
  assert_eq!(first_v2["results"], legacy["results"]);
  assert_eq!(first_v2["pagination"], json!({"current": 0, "has_more": true}));
  // A Rust API client can decode and re-encode the full response without
  // dropping nested or deprecated fields from either endpoint's media items.
  let typed_v2: ListMediaFilesForUserV2SuccessResponse = serde_json::from_value(first_v2.clone()).unwrap();
  assert_eq!(typed_v2.results.len(), 25);
  assert_eq!(serde_json::to_value(typed_v2).unwrap(), first_v2);
  let (_, last_v2) = get(&app, &format!("{v2}?include_user_uploads=true&page_index=1"), session).await;
  assert_eq!(last_v2["results"], last["results"]);
  assert_eq!(last_v2["pagination"], json!({"current": 1, "has_more": false}));
  let (_, exact_v2) = get(&app, &format!("{v2}?include_user_uploads=true&page_size=26"), session).await;
  assert_eq!(exact_v2["pagination"], json!({"current": 0, "has_more": false}));
  let (_, empty_v2) = get(&app, &format!("{v2}?include_user_uploads=true&page_index=2"), session).await;
  assert_eq!(empty_v2["results"], json!([]));
  assert_eq!(empty_v2["pagination"], json!({"current": 2, "has_more": false}));

  for operation in ["list", "list_v2"] {
    let path = format!(
      "/v1/media_files/{operation}/user/{}?include_user_uploads=true",
      owner.username
    );
    let (status, anonymous) = get(&app, &path, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
      anonymous["results"],
      json!([]),
      "private fixtures must require their owner session"
    );
    let (status, missing) = get(
      &app,
      &format!("/v1/media_files/{operation}/user/missing_fixture?include_user_uploads=true"),
      session,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(missing["results"], json!([]));
    let (status, filtered) = get(&app, &format!("{path}&filter_media_classes=image"), session).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(filtered["results"], json!([]));
  }
  for query in [
    "page_size=0".to_string(),
    "page_size=101".to_string(),
    format!("page_size=60&page_index={}", usize::MAX),
  ] {
    let (status, _) = get(&app, &format!("{v2}?{query}"), session).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
  }
  drop(app);
  db.destroy().await;
}

async fn get<S, B>(app: &S, path: &str, maybe_session: Option<&str>) -> (StatusCode, Value)
where
  S: Service<Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
  B: MessageBody,
  B::Error: std::fmt::Debug,
{
  let mut request = test::TestRequest::get().uri(path);
  if let Some(session) = maybe_session {
    request = request.insert_header(("session", session));
  }
  let response = test::call_service(app, request.to_request()).await;
  let status = response.status();
  (status, test::read_body_json(response).await)
}

fn assert_legacy_item_keys(item: &Value) {
  let mut actual = item.as_object().unwrap().keys().map(String::as_str).collect::<Vec<_>>();
  actual.sort_unstable();
  let mut expected = LEGACY_ITEM_FIELDS.to_vec();
  expected.sort_unstable();
  assert_eq!(actual, expected);
}
