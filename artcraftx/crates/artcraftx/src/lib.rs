pub mod commands;
pub mod credentials;
pub mod database;
pub mod error;
pub mod events;
pub mod lifecycle;
pub mod services;
pub mod state;
pub mod threads;
pub mod utils;
pub mod version;
pub mod windows;

use tauri::Manager;

use crate::commands::credentials::add_api_credential_command::add_api_credential_command;
use crate::commands::credentials::add_web_credential_command::add_web_credential_command;
use crate::commands::credentials::artcraft_login_command::artcraft_login_command;
use crate::commands::credentials::delete_credentials_command::delete_credentials_command;
use crate::commands::credentials::edit_api_credential_command::edit_api_credential_command;
use crate::commands::credentials::edit_web_credential_command::edit_web_credential_command;
use crate::commands::credentials::list_credentials_command::list_credentials_command;
use crate::commands::credentials::open_web_login_command::open_web_login_command;
use crate::commands::credentials::refresh_grok_statsig_command::refresh_grok_statsig_command;
use crate::commands::service::app_preferences::get_app_preferences_command::get_app_preferences_command;
use crate::commands::service::promptbox::get_promptbox_state_command::get_promptbox_state_command;
use crate::commands::service::promptbox::update_promptbox_state_command::update_promptbox_state_command;
use crate::commands::service::app_preferences::load_custom_sound_command::load_custom_sound_command;
use crate::commands::service::app_preferences::update_app_preference_command::update_app_preferences_command;
use crate::commands::service::app_preferences::update_backup_preference_command::update_backup_preference_command;
use crate::commands::service::app_preferences::update_prompt_preference_command::update_prompt_preference_command;
use crate::commands::service::app_preferences::update_sound_preference_command::update_sound_preference_command;
use crate::commands::cost_estimate::estimate_audio_cost_command::estimate_audio_cost_command;
use crate::commands::cost_estimate::estimate_image_cost_command::estimate_image_cost_command;
use crate::commands::cost_estimate::estimate_mesh_cost_command::estimate_mesh_cost_command;
use crate::commands::cost_estimate::estimate_splat_cost_command::estimate_splat_cost_command;
use crate::commands::cost_estimate::estimate_video_cost_command::estimate_video_cost_command;
use crate::commands::generate::models::audio::list_audio_models_command::list_audio_models_command;
use crate::commands::generate::models::image::list_image_models_command::list_image_models_command;
use crate::commands::generate::models::mesh::list_mesh_models_command::list_mesh_models_command;
use crate::commands::generate::models::splat::list_splat_models_command::list_splat_models_command;
use crate::commands::generate::models::video::list_video_models_command::list_video_models_command;
use crate::commands::download::download_directory_reveal_command::download_directory_reveal_command;
use crate::commands::download::download_media_file_command::download_media_file_command;
use crate::commands::download::download_url_command::download_url_command;
use crate::commands::download::open_local_file_command::open_local_file_command;
use crate::commands::generate::generate_audio::generate_audio_command::generate_audio_command;
use crate::commands::generate::generate_image::generate_image_command::generate_image_command;
use crate::commands::generate::generate_mesh::generate_mesh_command::generate_mesh_command;
use crate::commands::generate::generate_splat::generate_splat_command::generate_splat_command;
use crate::commands::generate::generate_video::generate_video_command::generate_video_command;
use crate::commands::service::get_app_info_command::get_app_info_command;
use crate::commands::service::load_without_cors_command::load_without_cors_command;
use crate::commands::service::platform_info_command::platform_info_command;
use crate::commands::local_files::get_local_thumbnails_command::get_local_thumbnails_command;
use crate::commands::task_queue::get_task_queue_command::get_task_queue_command;
use crate::commands::task_queue::mark_task_as_dismissed_command::mark_task_as_dismissed_command;
use crate::commands::task_queue::tasks_nuke_all_command::tasks_nuke_all_command;
use crate::lifecycle::startup::handle_tauri_startup::handle_tauri_startup;
use crate::lifecycle::startup::setup_main_window::setup_main_window;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::promptbox::promptbox_state_manager::PromptboxStateManager;
use crate::state::runtime::artcraft_platform_info::ArtcraftPlatformInfo;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::services::grok::state::grok_credential_manager::GrokCredentialManager;
use crate::services::grok::state::grok_websockets::GrokWebsockets;
use crate::services::midjourney::state::midjourney_credential_manager::MidjourneyCredentialManager;
use crate::services::midjourney::state::midjourney_live_session::MidjourneyLiveSession;
use crate::services::storyteller::state::storyteller_credential_manager::StorytellerCredentialManager;
use log::error;

use crate::state::usage_tracker::artcraft_usage_tracker::ArtcraftUsageTracker;
use tauri_plugin_dialog;
use tauri_plugin_http;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  // NB: Tauri wants to install the logger itself, so we can't rely on the logger crate
  // until the tauri runtime begins.
  println!("Loading config...");
  let app_data_root = AppDataRoot::create_default().expect("data directory should be created");
  let app_data_root_2 = app_data_root.clone();

  println!("Getting platform info...");
  let artcraft_platform_info = ArtcraftPlatformInfo::get();
  let artcraft_platform_info_2 = artcraft_platform_info.clone();

  println!("Platform info: {:?}", artcraft_platform_info);

  println!("Loading app preferences...");
  let app_preferences = AppPreferencesManager::load_or_default(&app_data_root);
  let promptbox_state = PromptboxStateManager::load_or_default(&app_data_root);
  let app_preferences_2 = app_preferences.clone();
  

  // NB: tauri-plugin-http stores the credentials on disk, so we can defer to that for now.
  // println!("Attempting to read existing artcraft credentials...");
  // let storyteller_creds_manager = StorytellerCredentialManager::initialize_from_disk_infallible(&app_data_root);
  let storyteller_creds_manager = StorytellerCredentialManager::initialize_empty(&app_data_root);
  let storyteller_creds_manager_2 = storyteller_creds_manager.clone();
  let storyteller_creds_manager_3 = storyteller_creds_manager.clone();
  
  let midjourney_creds_manager = MidjourneyCredentialManager::initialize_from_disk_infallible(&app_data_root);

  // In-memory, process-lifetime Midjourney session (user_id, websocket token,
  // live websocket handle). Shared between the enqueue command and the
  // completion threads.
  let midjourney_live_session = MidjourneyLiveSession::new();
  let midjourney_live_session_2 = midjourney_live_session.clone();

  let grok_creds_manager = GrokCredentialManager::initialize_from_disk_infallible(&app_data_root);
  let grok_creds_manager_2 = grok_creds_manager.clone();

  // One live Grok imagine websocket per account, shared by the enqueue command
  // and the third-party task polling thread.
  let grok_websockets = GrokWebsockets::new();
  let grok_websockets_2 = grok_websockets.clone();

  let artcraft_usage_tracker = ArtcraftUsageTracker::new();
  let artcraft_usage_tracker_2 = artcraft_usage_tracker.clone();

  println!("Initializing backend runtime...");

  let builder = tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_http::init())
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_upload::init())
    .setup(move |app| {
      // TODO(bt): This is broken on windows
      // log_environment_details();

      //if cfg!(debug_assertions) {
      //  app.handle().plugin(
      //    tauri_plugin_log::Builder::default()
      //      .level(log::LevelFilter::Info)
      //      .build(),
      //  )?;
      //}
      let app = app.handle().clone();
      let handle = app.clone();
      let root = app_data_root_2.clone();
      let storyteller_creds = storyteller_creds_manager_2.clone();

      tauri::async_runtime::block_on(async move {
        let _result = setup_main_window(&app).await;

        let result = handle_tauri_startup(
          handle,
          root,
          app_preferences_2,
          artcraft_platform_info_2,
          artcraft_usage_tracker_2,
          storyteller_creds,
          midjourney_live_session_2,
          grok_creds_manager_2,
          grok_websockets_2,
        ).await;

        if let Err(err) = result {
          error!("Failed to handle Tauri startup: {:?}", err);
          panic!("Failed to handle Tauri startup: {:?}", err);
        }
      });

      Ok(())
    })
    .manage(app_data_root)
    .manage(app_preferences)
    .manage(promptbox_state)
    .manage(artcraft_platform_info)
    .manage(artcraft_usage_tracker)
    .manage(grok_creds_manager)
    .manage(grok_websockets)
    .manage(midjourney_creds_manager)
    .manage(midjourney_live_session)
    .manage(storyteller_creds_manager_3);

  // TODO: Break this out into another module, because RustRover/IntelliJ lags with these macros.
  //  My first attempt at naively doing this didn't work because the macros can't find their codegen'd targets.
  let builder = builder.invoke_handler(tauri::generate_handler![
    add_api_credential_command,
    add_web_credential_command,
    artcraft_login_command,
    delete_credentials_command,
    edit_api_credential_command,
    edit_web_credential_command,
    list_credentials_command,
    open_local_file_command,
    open_web_login_command,
    refresh_grok_statsig_command,
    download_directory_reveal_command,
    download_media_file_command,
    download_url_command,
    estimate_audio_cost_command,
    estimate_image_cost_command,
    estimate_mesh_cost_command,
    estimate_splat_cost_command,
    estimate_video_cost_command,
    list_audio_models_command,
    list_image_models_command,
    list_mesh_models_command,
    list_splat_models_command,
    list_video_models_command,
    generate_audio_command,
    generate_image_command,
    generate_mesh_command,
    generate_splat_command,
    generate_video_command,
    get_app_info_command,
    get_app_preferences_command,
    get_promptbox_state_command,
    get_local_thumbnails_command,
    get_task_queue_command,
    load_custom_sound_command,
    load_without_cors_command,
    mark_task_as_dismissed_command,
    platform_info_command,
    tasks_nuke_all_command,
    update_app_preferences_command,
    update_backup_preference_command,
    update_prompt_preference_command,
    update_promptbox_state_command,
    update_sound_preference_command,
  ]);

  builder.run(tauri::generate_context!("tauri.conf.json"))
    .expect("error while running tauri application");
}
