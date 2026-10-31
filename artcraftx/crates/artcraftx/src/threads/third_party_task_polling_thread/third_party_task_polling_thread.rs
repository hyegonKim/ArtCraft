use crate::services::grok::state::grok_websockets::GrokWebsockets;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::database::task_database::TaskDatabase;
use crate::threads::third_party_task_polling_thread::handlers::fal::poll_fal_tasks::poll_fal_tasks;
use crate::threads::third_party_task_polling_thread::handlers::grok::grok_image_collector::GrokImageCollector;
use crate::threads::third_party_task_polling_thread::handlers::grok::poll_grok_image_tasks::poll_grok_image_tasks;
use crate::threads::third_party_task_polling_thread::handlers::higgsfield::higgsfield_poll_sessions::HiggsfieldPollSessions;
use crate::threads::third_party_task_polling_thread::handlers::higgsfield::poll_higgsfield_tasks::poll_higgsfield_tasks;
use crate::database::task_database_pending_statuses::TASK_DATABASE_PENDING_STATUSES;
use crate::services::storyteller::state::storyteller_credential_manager::StorytellerCredentialManager;
use core_types::enums::generation_source::GenerationSource;
use sqlite_identifiers::enums::task_type::TaskType;
use log::{error, info, warn};
use sqlite_database::queries::read::list_non_artcraft_pending_tasks::{
  list_non_artcraft_pending_tasks, ListNonArtcraftPendingTasksArgs,
};
use sqlite_database::queries::task::Task;
use std::time::Duration;
use tauri::AppHandle;

const SLEEP_NO_THIRD_PARTY_JOBS_SEEN: Duration = Duration::from_secs(10);
const SLEEP_THIRD_PARTY_JOBS_SEEN: Duration = Duration::from_secs(2);
const SLEEP_BETWEEN_FAL_POLLS: Duration = Duration::from_secs(1);
const SLEEP_ON_ERROR: Duration = Duration::from_secs(30);

pub async fn third_party_task_polling_thread(
  app_handle: AppHandle,
  app_data_root: AppDataRoot,
  app_preferences: AppPreferencesManager,
  task_database: TaskDatabase,
  storyteller_creds_manager: StorytellerCredentialManager,
  grok_websockets: GrokWebsockets,
) -> ! {
  let mut has_ever_seen_third_party_jobs = false;
  // Grok streams a prompt's images one frame at a time; accumulate across iterations.
  let mut grok_image_collector = GrokImageCollector::new();
  // One Higgsfield session per account, reused so bearer tokens aren't re-minted every poll.
  let mut higgsfield_sessions = HiggsfieldPollSessions::new();

  loop {
    let result = poll_iteration(
      &app_handle,
      &app_data_root,
      &app_preferences,
      &task_database,
      &storyteller_creds_manager,
      &grok_websockets,
      &mut grok_image_collector,
      &mut higgsfield_sessions,
      &mut has_ever_seen_third_party_jobs,
    ).await;

    if let Err(err) = result {
      error!("[ThirdPartyPolling] Error in polling loop: {:?}", err);
      tokio::time::sleep(SLEEP_ON_ERROR).await;
    }
  }
}

async fn poll_iteration(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  storyteller_creds_manager: &StorytellerCredentialManager,
  grok_websockets: &GrokWebsockets,
  grok_image_collector: &mut GrokImageCollector,
  higgsfield_sessions: &mut HiggsfieldPollSessions,
  has_ever_seen_third_party_jobs: &mut bool,
) -> Result<(), PollError> {
  let task_list = list_non_artcraft_pending_tasks(ListNonArtcraftPendingTasksArgs {
    db: task_database.get_connection(),
    task_statuses: &TASK_DATABASE_PENDING_STATUSES,
  }).await?;

  let tasks = task_list.tasks;

  if tasks.is_empty() {
    let sleep_duration = if *has_ever_seen_third_party_jobs {
      SLEEP_THIRD_PARTY_JOBS_SEEN
    } else {
      SLEEP_NO_THIRD_PARTY_JOBS_SEEN
    };
    tokio::time::sleep(sleep_duration).await;
    return Ok(());
  }

  *has_ever_seen_third_party_jobs = true;

  let fal_tasks: Vec<&Task> = tasks.iter()
    .filter(|t| t.provider == GenerationSource::Fal)
    .collect();

  // First-party Grok Imagine images (websocket results; see handlers/grok).
  let grok_image_tasks: Vec<&Task> = tasks.iter()
    .filter(|t| t.provider == GenerationSource::Grok && t.task_type == TaskType::ImageGeneration)
    .collect();

  // First-party Higgsfield images and videos (see handlers/higgsfield).
  let higgsfield_tasks: Vec<&Task> = tasks.iter()
    .filter(|t| t.provider == GenerationSource::Higgsfield)
    .collect();

  let unhandled_tasks: Vec<&Task> = tasks.iter()
    .filter(|t| t.provider != GenerationSource::Fal)
    .filter(|t| t.provider != GenerationSource::Higgsfield)
    .filter(|t| !(t.provider == GenerationSource::Grok && t.task_type == TaskType::ImageGeneration))
    .collect();

  for task in &unhandled_tasks {
    warn!(
      "[ThirdPartyPolling] Skipping unhandled task: id={}, provider={:?}, type={:?}",
      task.id.as_str(),
      task.provider,
      task.task_type,
    );
  }

  if !grok_image_tasks.is_empty() {
    info!("[ThirdPartyPolling] {} Grok image job(s) pending", grok_image_tasks.len());
    poll_grok_image_tasks(
      app_handle,
      app_data_root,
      app_preferences,
      task_database,
      storyteller_creds_manager,
      grok_websockets,
      grok_image_collector,
      &grok_image_tasks,
    ).await;
  }

  if !higgsfield_tasks.is_empty() {
    info!("[ThirdPartyPolling] {} Higgsfield job(s) pending", higgsfield_tasks.len());
    poll_higgsfield_tasks(
      app_handle,
      app_data_root,
      app_preferences,
      task_database,
      storyteller_creds_manager,
      higgsfield_sessions,
      &higgsfield_tasks,
    ).await;
  }

  if fal_tasks.is_empty() {
    // NB: the Grok poll already listened on its sockets for a couple of seconds.
    if grok_image_tasks.is_empty() {
      tokio::time::sleep(SLEEP_THIRD_PARTY_JOBS_SEEN).await;
    }
    return Ok(());
  }

  info!("[ThirdPartyPolling] {} FAL job(s) ready to check", fal_tasks.len());

  poll_fal_tasks(
    app_handle,
    app_data_root,
    app_preferences,
    task_database,
    storyteller_creds_manager,
    &fal_tasks,
  ).await;

  tokio::time::sleep(SLEEP_BETWEEN_FAL_POLLS).await;

  Ok(())
}

// ── Error ──

#[derive(Debug)]
enum PollError {
  SqliteTasksError(sqlite_database::error::SqliteTasksError),
}

impl From<sqlite_database::error::SqliteTasksError> for PollError {
  fn from(err: sqlite_database::error::SqliteTasksError) -> Self {
    Self::SqliteTasksError(err)
  }
}

impl std::fmt::Display for PollError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::SqliteTasksError(err) => write!(f, "SQLite error: {:?}", err),
    }
  }
}
