use crate::services::storyteller::state::storyteller_credential_manager::StorytellerCredentialManager;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::services::storyteller::threads::storyteller_task_polling_thread::handle_storyteller_failed_job::handle_failed_job;
use crate::services::storyteller::threads::storyteller_task_polling_thread::handle_storyteller_successful_job::handle_successful_job;
use crate::state::database::task_database::TaskDatabase;
use anyhow::anyhow;
use artcraft_client::endpoints::jobs::batch_get_job_status::batch_get_job_status;
use artcraft_client::enums::common::job_status_plus::JobStatusPlus;
use artcraft_client::error::api_error::ApiError;
use artcraft_client::error::storyteller_error::StorytellerError;
use artcraft_client::tokens::generic_inference_jobs::InferenceJobToken;
use artcraft_client::utils::api_host::ApiHost;
use core_types::enums::generation_source::GenerationSource;
use errors::AnyhowResult;
use log::error;
use sqlite_database::queries::read::list_tasks_by_provider_and_status::{list_tasks_by_provider_and_status, ListTasksByProviderAndStatusArgs};
use sqlite_database::queries::task::Task;
use sqlite_identifiers::enums::task_status::TaskStatus;
use std::collections::{HashMap, HashSet};
use tauri::AppHandle;

pub async fn storyteller_task_polling_thread(
  app_handle: AppHandle,
  app_data_root: AppDataRoot,
  app_preferences: AppPreferencesManager,
  task_database: TaskDatabase,
  storyteller_creds_manager: StorytellerCredentialManager,
) -> ! {
  loop {
    let res = polling_loop(
      &app_handle,
      &app_data_root,
      &app_preferences,
      &task_database,
      &storyteller_creds_manager,
    ).await;
    if let Err(err) = res {
      error!("An error occurred: {:?}", err);
    }
    // NB: Only sleep if an error occurs.
    tokio::time::sleep(std::time::Duration::from_millis(30_000)).await;
  }
}

async fn polling_loop(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  storyteller_creds_manager: &StorytellerCredentialManager,
) -> AnyhowResult<()> {
  loop {
    // Wait before next request for jobs.
    tokio::time::sleep(std::time::Duration::from_millis(5_000)).await;

    // Only poll for tasks that are still in flight.
    let unfinished_statuses = HashSet::from([TaskStatus::Pending, TaskStatus::Started]);

    let tasks = list_tasks_by_provider_and_status(ListTasksByProviderAndStatusArgs {
      db: task_database.get_connection(),
      provider: GenerationSource::Artcraft,
      task_statuses: &unfinished_statuses,
    }).await?;

    let tasks_by_provider_job_id = tasks.tasks.iter()
        .filter_map(|task| {
          task.provider_job_id
              .as_ref()
              .map(|provider_job_id| (provider_job_id.clone(), task.clone()))
        })
        .collect::<HashMap<String, Task>>();

    if tasks_by_provider_job_id.is_empty() {
      continue; // Nothing in flight.
    }

    let job_tokens = tasks_by_provider_job_id.keys()
        .map(|job_id| InferenceJobToken::new_from_str(job_id))
        .collect::<Vec<_>>();

    let creds = storyteller_creds_manager.get_credentials()?;

    let result = batch_get_job_status(
      &ApiHost::Storyteller,
      creds.as_ref(),
      &job_tokens,
    ).await;

    let job_states = match result {
      Ok(result) => result.job_states,
      Err(err) => {
        match &err {
          StorytellerError::Api(ApiError::TooManyRequests(message)) => {
            error!("Too many requests (sleeping): {:?}", message);
            tokio::time::sleep(std::time::Duration::from_millis(60_000)).await;
          }
          _ => {}
        }
        return Err(anyhow!(err));
      }
    };

    for job in job_states.iter() {
      let task = match tasks_by_provider_job_id.get(job.job_token.as_str()) {
        Some(task) => task,
        None => continue,
      };

      match job.status.status {
        JobStatusPlus::CompleteSuccess => {
          handle_successful_job(app_handle, app_data_root, app_preferences, creds.as_ref(), job, task, task_database).await?;
        }
        JobStatusPlus::CompleteFailure | JobStatusPlus::Dead => {
          handle_failed_job(app_handle, job, task, task_database).await?;
        }
        _ => continue,
      }
    }

  }
}
