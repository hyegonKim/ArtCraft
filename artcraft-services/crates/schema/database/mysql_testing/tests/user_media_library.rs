//! Executes the actual library queries against migrated, disposable local MySQL.
//! IsolatedTestDatabase uses a fixed Unix socket, never DATABASE_URL/MYSQL_URL.
use std::collections::HashSet;
use std::time::Instant;

use enums::by_table::entity_stats::stats_entity_type::StatsEntityType;
use enums::by_table::media_files::media_file_class::MediaFileClass;
use enums::by_table::media_files::media_file_engine_category::MediaFileEngineCategory;
use enums::by_table::media_files::media_file_origin_category::MediaFileOriginCategory;
use enums::by_table::media_files::media_file_origin_product_category::MediaFileOriginProductCategory;
use enums::by_table::media_files::media_file_type::MediaFileType;
use enums::by_table::prompts::prompt_type::PromptType;
use enums::common::view_as::ViewAs;
use enums::common::visibility::Visibility;
use mysql_queries::queries::media_files::list::count_media_files_for_user::{
  count_media_files_for_user, CountMediaFilesForUserArgs,
};
use mysql_queries::queries::media_files::list::list_media_files_for_user::{
  list_media_files_for_user, ListMediaFileForUserArgs,
};
use mysql_queries::queries::media_files::list::user_media_file_filters::UserMediaFileFilters;
use mysql_testing::fixtures::users::create_test_user;
use mysql_testing::isolated::IsolatedTestDatabase;
use sqlx::{MySql, MySqlPool, QueryBuilder};
use tokens::tokens::users::UserToken;

const CREATED_AT: &str = "2025-01-01 12:00:00";

#[tokio::test]
async fn user_media_library_preserves_filters_visibility_joins_and_pagination() {
  let db = IsolatedTestDatabase::create().await;
  let owner = create_test_user(&db.pool).await.unwrap();
  let other = create_test_user(&db.pool).await.unwrap();
  let rows = [
    ("m_public_image", MediaFileClass::Image, MediaFileType::Png, Visibility::Public, true),
    ("m_private_video", MediaFileClass::Video, MediaFileType::Mp4, Visibility::Private, false),
    ("m_hidden_audio", MediaFileClass::Audio, MediaFileType::Mp3, Visibility::Hidden, false),
    ("m_public_mesh", MediaFileClass::Mesh, MediaFileType::Glb, Visibility::Public, false),
    ("m_private_splat", MediaFileClass::Splat, MediaFileType::Spz, Visibility::Private, false),
    ("m_legacy_mesh", MediaFileClass::Dimensional, MediaFileType::Glb, Visibility::Public, false),
  ];
  for (token, class, kind, visibility, upload) in rows {
    insert_fixture(&db.pool, Fixture { owner: &owner.user_token, token, class, kind, visibility, upload }).await;
  }
  for token in ["m_intermediate", "m_user_deleted", "m_mod_deleted"] {
    insert_fixture(&db.pool, Fixture::image(&owner.user_token, token)).await;
  }
  insert_fixture(&db.pool, Fixture::image(&other.user_token, "m_other_user")).await;
  sqlx::query("UPDATE media_files SET is_intermediate_system_file = true WHERE token = 'm_intermediate'")
    .execute(&db.pool)
    .await
    .unwrap();
  sqlx::query("UPDATE media_files SET user_deleted_at = NOW() WHERE token = 'm_user_deleted'")
    .execute(&db.pool)
    .await
    .unwrap();
  sqlx::query("UPDATE media_files SET mod_deleted_at = NOW() WHERE token = 'm_mod_deleted'")
    .execute(&db.pool)
    .await
    .unwrap();
  sqlx::query("UPDATE media_files SET maybe_engine_category = ? WHERE token = 'm_public_mesh'")
    .bind(MediaFileEngineCategory::Object.to_str())
    .execute(&db.pool)
    .await
    .unwrap();
  sqlx::query("INSERT INTO prompts (token, prompt_type, creator_ip_address, maybe_other_args) VALUES ('p_library', ?, '127.0.0.1', ?)")
    .bind(PromptType::ArtcraftApp.to_str()).bind(r#"{"fd":true}"#).execute(&db.pool).await.unwrap();
  sqlx::query("UPDATE media_files SET maybe_cover_image_media_file_token = 'm_intermediate', maybe_prompt_token = 'p_library', maybe_duration_millis = 1234, maybe_title = 'Fixture title' WHERE token = 'm_private_video'").execute(&db.pool).await.unwrap();
  sqlx::query("INSERT INTO entity_stats (entity_type, entity_token, ratings_positive_count, bookmark_count) VALUES (?, 'm_private_video', 3, 7)")
    .bind(StatsEntityType::MediaFile.to_str()).execute(&db.pool).await.unwrap();

  let filters = filters(&owner.username);
  for (view_as, expected) in [
    (
      ViewAs::Author,
      vec!["m_legacy_mesh", "m_private_splat", "m_public_mesh", "m_hidden_audio", "m_private_video", "m_public_image"],
    ),
    (
      ViewAs::Moderator,
      vec!["m_legacy_mesh", "m_private_splat", "m_public_mesh", "m_hidden_audio", "m_private_video", "m_public_image"],
    ),
    (ViewAs::AnotherUser, vec!["m_legacy_mesh", "m_public_mesh", "m_public_image"]),
  ] {
    let selected = UserMediaFileFilters { view_as, ..filters };
    let records = list_media_files_for_user(ListMediaFileForUserArgs {
      filters: selected,
      limit: 60,
      offset: 0,
      sort_ascending: false,
      mysql_executor: &db.pool,
    })
    .await
    .unwrap();
    assert_eq!(records.iter().map(|r| r.token.as_str()).collect::<Vec<_>>(), expected);
    assert_eq!(
      count_media_files_for_user(CountMediaFilesForUserArgs { filters: selected, mysql_executor: &db.pool })
        .await
        .unwrap(),
      expected.len() as i64
    );
  }
  let records = list_media_files_for_user(ListMediaFileForUserArgs {
    filters,
    limit: 60,
    offset: 0,
    sort_ascending: true,
    mysql_executor: &db.pool,
  })
  .await
  .unwrap();
  assert_eq!(records.first().unwrap().token.as_str(), "m_public_image");
  let video = &records[1];
  assert_eq!(video.maybe_file_cover_image_public_bucket_hash.as_deref(), Some("fixture_hash"));
  assert_eq!(video.maybe_prompt_args.as_ref().unwrap().used_face_detailer, Some(true));
  assert_eq!(video.maybe_ratings_positive_count, Some(3));
  assert_eq!(video.maybe_bookmark_count, Some(7));
  assert_eq!(video.maybe_duration_millis, Some(1234));
  assert_eq!(video.maybe_title.as_deref(), Some("Fixture title"));

  // All timestamps tie. The id tiebreaker must avoid duplicates/omissions.
  let mut paged = vec![];
  for offset in [0, 2, 4, 6] {
    let records = list_media_files_for_user(ListMediaFileForUserArgs {
      filters,
      limit: 2,
      offset,
      sort_ascending: true,
      mysql_executor: &db.pool,
    })
    .await
    .unwrap();
    paged.extend(records.into_iter().map(|r| r.token.to_string()));
  }
  assert_eq!(paged, rows.iter().map(|r| r.0.to_string()).collect::<Vec<_>>());
  let classes = HashSet::from([MediaFileClass::Image, MediaFileClass::Video]);
  let types = HashSet::from([MediaFileType::Glb]);
  let engines = HashSet::from([MediaFileEngineCategory::Object]);
  let empty = HashSet::new();
  for (selected, expected) in [
    (UserMediaFileFilters { maybe_filter_media_classes: Some(&classes), ..filters }, 2),
    (UserMediaFileFilters { maybe_filter_media_types: Some(&types), ..filters }, 2),
    (UserMediaFileFilters { maybe_filter_engine_categories: Some(&engines), ..filters }, 1),
    (UserMediaFileFilters { include_user_uploads: false, ..filters }, 5),
    (UserMediaFileFilters { maybe_filter_media_classes: Some(&empty), ..filters }, 6),
    (UserMediaFileFilters { username: "' OR 1=1 --", ..filters }, 0),
  ] {
    let records = list_media_files_for_user(ListMediaFileForUserArgs {
      filters: selected,
      limit: 60,
      offset: 0,
      sort_ascending: false,
      mysql_executor: &db.pool,
    })
    .await
    .unwrap();
    assert_eq!(records.len(), expected);
    assert_eq!(
      count_media_files_for_user(CountMediaFilesForUserArgs { filters: selected, mysql_executor: &db.pool })
        .await
        .unwrap(),
      expected as i64
    );
  }
  // Queries must honor an existing transaction, including uncommitted changes.
  let mut tx = db.pool.begin().await.unwrap();
  sqlx::query("UPDATE media_files SET user_deleted_at = NOW() WHERE token = 'm_private_video'")
    .execute(&mut *tx)
    .await
    .unwrap();
  assert_eq!(
    count_media_files_for_user(CountMediaFilesForUserArgs { filters, mysql_executor: &mut *tx }).await.unwrap(),
    5
  );
  assert_eq!(
    list_media_files_for_user(ListMediaFileForUserArgs {
      filters,
      limit: 60,
      offset: 0,
      sort_ascending: false,
      mysql_executor: &mut *tx
    })
    .await
    .unwrap()
    .len(),
    5
  );
  tx.rollback().await.unwrap();
  assert_eq!(
    count_media_files_for_user(CountMediaFilesForUserArgs { filters, mysql_executor: &db.pool }).await.unwrap(),
    6
  );
  db.destroy().await;
}

#[tokio::test]
#[ignore = "large local fixture; run explicitly with --ignored --nocapture"]
async fn user_media_library_large_page_reads_are_bounded() {
  let db = IsolatedTestDatabase::create().await;
  let owner = create_test_user(&db.pool).await.unwrap();
  let other = create_test_user(&db.pool).await.unwrap();
  // 60k owner rows, followed by 240k newer files from another account.
  for chunk in 0..300 {
    let mut query = QueryBuilder::<MySql>::new("INSERT INTO media_files (token, origin_category, origin_product_category, media_type, media_class, checksum_sha2, public_bucket_directory_hash, creator_ip_address, maybe_creator_user_token, creator_set_visibility, is_intermediate_system_file, is_user_upload, created_at) ");
    query.push_values(0..1000, |mut row, index| {
      row
        .push_bind(format!("m_bench_{}", chunk * 1000 + index))
        .push_bind(MediaFileOriginCategory::Upload.to_str())
        .push_bind(MediaFileOriginProductCategory::Unknown.to_str())
        .push_bind(MediaFileType::Png.to_str())
        .push_bind(MediaFileClass::Image.to_str())
        .push_bind("0".repeat(64))
        .push_bind("fixture_hash")
        .push_bind("127.0.0.1")
        .push_bind(if chunk < 60 { owner.user_token.as_str() } else { other.user_token.as_str() })
        .push_bind(Visibility::Private.to_str())
        .push_bind(false)
        .push_bind(true)
        .push_bind(CREATED_AT);
    });
    query.build().execute(&db.pool).await.unwrap();
  }
  sqlx::query("ANALYZE TABLE media_files").execute(&db.pool).await.unwrap();
  let classes = HashSet::from([
    MediaFileClass::Image,
    MediaFileClass::Video,
    MediaFileClass::Audio,
    MediaFileClass::Dimensional,
    MediaFileClass::Mesh,
    MediaFileClass::Splat,
  ]);
  let filters = UserMediaFileFilters { maybe_filter_media_classes: Some(&classes), ..filters(&owner.username) };
  let mut conn = db.pool.acquire().await.unwrap();
  let before = scan_reads(&mut conn).await;
  let start = Instant::now();
  let records = list_media_files_for_user(ListMediaFileForUserArgs {
    filters,
    limit: 61,
    offset: 0,
    sort_ascending: false,
    mysql_executor: &mut *conn,
  })
  .await
  .unwrap();
  let elapsed = start.elapsed();
  let scanned = scan_reads(&mut conn).await - before;
  assert_eq!(records.len(), 61);
  assert_eq!(records.first().unwrap().token.as_str(), "m_bench_59999");
  assert!(scanned < 200, "page query scanned {scanned} sequential index entries");
  let start = Instant::now();
  assert_eq!(
    count_media_files_for_user(CountMediaFilesForUserArgs { filters, mysql_executor: &mut *conn }).await.unwrap(),
    60_000
  );
  println!(
    "300k fixtures / 60k owned: page={elapsed:?}, sequential index reads={scanned}; optional count={:?}",
    start.elapsed()
  );
  drop(conn);
  db.destroy().await;
}

struct Fixture<'a> {
  owner: &'a UserToken,
  token: &'a str,
  class: MediaFileClass,
  kind: MediaFileType,
  visibility: Visibility,
  upload: bool,
}

impl<'a> Fixture<'a> {
  fn image(owner: &'a UserToken, token: &'a str) -> Self {
    Self {
      owner,
      token,
      class: MediaFileClass::Image,
      kind: MediaFileType::Png,
      visibility: Visibility::Public,
      upload: true,
    }
  }
}

fn filters(username: &str) -> UserMediaFileFilters<'_> {
  UserMediaFileFilters {
    username,
    maybe_filter_media_types: None,
    maybe_filter_media_classes: None,
    maybe_filter_engine_categories: None,
    include_user_uploads: true,
    view_as: ViewAs::Author,
  }
}

async fn insert_fixture(pool: &MySqlPool, fixture: Fixture<'_>) {
  sqlx::query("INSERT INTO media_files (token, origin_category, origin_product_category, media_type, media_class, checksum_sha2, public_bucket_directory_hash, creator_ip_address, maybe_creator_user_token, creator_set_visibility, is_intermediate_system_file, is_user_upload, created_at) VALUES (?, ?, ?, ?, ?, ?, 'fixture_hash', '127.0.0.1', ?, ?, false, ?, ?)")
    .bind(fixture.token).bind(MediaFileOriginCategory::Upload.to_str()).bind(MediaFileOriginProductCategory::Unknown.to_str())
    .bind(fixture.kind.to_str()).bind(fixture.class.to_str()).bind("0".repeat(64)).bind(fixture.owner.as_str())
    .bind(fixture.visibility.to_str()).bind(fixture.upload).bind(CREATED_AT).execute(pool).await.unwrap();
}

async fn scan_reads(conn: &mut sqlx::MySqlConnection) -> u64 {
  let values: Vec<(String, String)> =
    sqlx::query_as("SHOW SESSION STATUS WHERE Variable_name IN ('Handler_read_next', 'Handler_read_prev')")
      .fetch_all(conn)
      .await
      .unwrap();
  values.into_iter().map(|(_, value)| value.parse::<u64>().unwrap()).sum()
}
