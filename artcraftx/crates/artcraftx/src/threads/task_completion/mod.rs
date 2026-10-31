//! The single place a finished generation is delivered, no matter which
//! provider produced it.
//!
//! Every poller (Storyteller aside — its files already live on the server)
//! only has to fetch the provider's result files into the temp directory and
//! call [`complete_task_with_local_files`]; saving to the user's download
//! directory, uploading to ArtCraft, marking the task complete, recording
//! where the files landed, and notifying the frontend all happen here.
//!
//! [`complete_task_with_local_files`]: complete_task_with_local_files::complete_task_with_local_files

pub mod complete_task_with_local_files;
pub mod save_results_to_download_dir;
pub mod upload_results_to_artcraft;
