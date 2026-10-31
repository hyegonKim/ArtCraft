//! Round-trip tests against a throwaway, freshly migrated SQLite database.
//!
//! Run with `SQLX_OFFLINE=true cargo test -p sqlite_database`.

use sqlite_database::connection::TaskDbConnection;
use sqlite_database::queries::create::create_task::{create_task, CreateTaskArgs};
use sqlite_database::queries::read::get_task_by_provider_and_provider_job_id::{
  get_task_by_provider_and_provider_job_id, GetTaskByProviderAndProviderJobIdArgs,
};
use sqlite_database::queries::read::list_tasks_for_frontend::list_tasks_for_frontend;
use sqlite_database::queries::update::update_successful_task_status_with_metadata::{
  update_successful_task_status_with_metadata, UpdateSuccessfulTaskArgs,
};
use sqlite_database::queries::update::update_task_download_locations::{
  update_task_download_locations, UpdateTaskDownloadLocationsArgs,
};
use core_types::enums::generation_source::GenerationSource;
use sqlite_identifiers::enums::task_media_file_class::TaskMediaFileClass;
use sqlite_identifiers::enums::task_status::TaskStatus;
use sqlite_identifiers::enums::task_type::TaskType;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use std::path::Path;

const PROVIDER_JOB_ID: &str = "job_test_123";

#[tokio::test]
async fn batch_flag_is_written_on_insert_and_read_back() {
  let (_dir, db) = fresh_database().await;

  let task_id = create_task(CreateTaskArgs {
    db: &db,
    status: TaskStatus::Pending,
    task_type: TaskType::ImageGeneration,
    model_type: None,
    provider: GenerationSource::Artcraft,
    provider_job_id: Some(PROVIDER_JOB_ID),
    is_batch_generation: true,
    queue_status_url: None,
    queue_response_url: None,
    prompt_token: None,
    frontend_caller: None,
    frontend_subscriber_id: None,
    frontend_subscriber_payload: None,
  }).await.unwrap();

  let task = get_task_by_provider_and_provider_job_id(GetTaskByProviderAndProviderJobIdArgs {
    db: &db,
    provider: GenerationSource::Artcraft,
    provider_job_id: PROVIDER_JOB_ID,
  }).await.unwrap().expect("task exists");
  assert_eq!(task.id, task_id);
  assert!(task.is_batch_generation);

  let listed = list_tasks_for_frontend(&db).await.unwrap();
  assert_eq!(listed.tasks.len(), 1);
  assert!(listed.tasks[0].is_batch_generation);
  assert_eq!(listed.tasks[0].on_complete_directory_location, None);
  assert_eq!(listed.tasks[0].on_complete_first_file_location, None);
}

#[tokio::test]
async fn download_locations_are_recorded_on_completion() {
  let (_dir, db) = fresh_database().await;

  let task_id = create_task(CreateTaskArgs {
    db: &db,
    status: TaskStatus::Pending,
    task_type: TaskType::VideoGeneration,
    model_type: None,
    provider: GenerationSource::Artcraft,
    provider_job_id: Some(PROVIDER_JOB_ID),
    is_batch_generation: false,
    queue_status_url: None,
    queue_response_url: None,
    prompt_token: None,
    frontend_caller: None,
    frontend_subscriber_id: None,
    frontend_subscriber_payload: None,
  }).await.unwrap();

  let media_token = MediaFileToken::new_from_str("m_test");
  let completed = update_successful_task_status_with_metadata(UpdateSuccessfulTaskArgs {
    db: &db,
    task_id: &task_id,
    maybe_batch_token: None,
    maybe_primary_media_file_token: Some(&media_token),
    maybe_primary_media_file_class: Some(TaskMediaFileClass::Video),
    maybe_primary_media_file_cdn_url: Some("https://cdn.example/out.mp4"),
    maybe_primary_media_file_thumbnail_url_template: None,
  }).await.unwrap();
  assert!(completed);

  let recorded = update_task_download_locations(UpdateTaskDownloadLocationsArgs {
    db: &db,
    task_id: &task_id,
    directory_location: Path::new("/Users/someone/Downloads/Artcraft"),
    first_file_location: Path::new("/Users/someone/Downloads/Artcraft/veo_3_2026-08-30.mp4"),
  }).await.unwrap();
  assert!(recorded);

  let listed = list_tasks_for_frontend(&db).await.unwrap();
  let task = &listed.tasks[0];
  assert_eq!(task.status, TaskStatus::CompleteSuccess);
  assert!(!task.is_batch_generation);
  assert_eq!(task.on_complete_directory_location.as_deref(), Some("/Users/someone/Downloads/Artcraft"));
  assert_eq!(
    task.on_complete_first_file_location.as_deref(),
    Some("/Users/someone/Downloads/Artcraft/veo_3_2026-08-30.mp4"),
  );
}

#[tokio::test]
async fn recording_locations_for_an_unknown_task_updates_nothing() {
  let (_dir, db) = fresh_database().await;
  let recorded = update_task_download_locations(UpdateTaskDownloadLocationsArgs {
    db: &db,
    task_id: &sqlite_identifiers::ids::task_id::TaskId::generate(),
    directory_location: Path::new("/tmp"),
    first_file_location: Path::new("/tmp/x.png"),
  }).await.unwrap();
  assert!(!recorded);
}

async fn fresh_database() -> (tempfile::TempDir, TaskDbConnection) {
  let dir = tempfile::tempdir().unwrap();
  let db = TaskDbConnection::connect_and_migrate(dir.path().join("tasks.sqlite")).await.unwrap();
  (dir, db)
}
