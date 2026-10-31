//! lru-delete-models
//!
//! Amazon EFS is costing us a ton of money and we need a quick way to stop the bleeding.
//!

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, SystemTime};

use clap::Parser;
use walkdir::WalkDir;

use errors::AnyhowResult;
use logging::{error, info, init_env_logger, TYPICAL_LOG_LEVEL};

/// TTS models start with this prefix, eg. "TM:r60rp93g7d1x"
const TTS_MODEL_PREFIX : &str = "TM:";

/// VM models start with this prefix, eg. "VM:8jbc6dh0b77d"
const VOCODER_MODEL_PREFIX : &str = "VM:";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
  // Path to TTS models and vocoder models to clear
  // The script will recursively scan the directories.
  #[arg(short, long)]
  directory: String,

  // Number of models to keep
  #[arg(short, long, default_value_t = 100)]
  keep: usize,

  // Only perform the deletes when this is set.
  #[arg(short, long, default_value_t = false)]
  confirm: bool,
}

#[derive(Debug)]
struct FileData {
  path: PathBuf,
  last_access: SystemTime,
}

pub fn main() -> AnyhowResult<()> {
  init_env_logger(Some(TYPICAL_LOG_LEVEL));

  info!("lru-delete-models");

  let args = CliArgs::parse();

  delete_files_loop(args);

  Ok(())
}

fn delete_files_loop(args: CliArgs) {
  loop {
    let files = find_model_files(&args.directory);
    let total_file_count = files.len();

    let files = scope_to_files_to_remove(files, args.keep);

    info!("Files total: {total_file_count} ; to delete: {}", files.len());

    for file in files {
      if !args.confirm {
        info!("would have removed file (use --confirm to delete): {:?}", file);
      } else {
        info!("removing file: {:?}", file);

        if let Err(err) = std::fs::remove_file(file) {
          error!("Error removing file: {err}");
        }
      }

      thread::sleep(Duration::from_millis(100));
    }

    thread::sleep(Duration::from_secs(10));
  }
}

fn find_model_files(directory: &str) -> Vec<PathBuf> {
  WalkDir::new(directory).into_iter()
      .filter_map(Result::ok)
      .filter(|e| !e.file_type().is_dir())
      .map(|e| e.path().canonicalize())
      .filter_map(Result::ok)
      .filter(|path| {
        path.file_name()
            .and_then(|f| f.to_str())
            .map(|f| f.starts_with(TTS_MODEL_PREFIX) || f.starts_with(VOCODER_MODEL_PREFIX))
            //.map(|f| f.ends_with(".jpg") || f.starts_with("README"))
            .unwrap_or(false)
      })
      .collect::<Vec<_>>()
}

fn scope_to_files_to_remove(files: Vec<PathBuf>, keep_count: usize) -> Vec<PathBuf> {
  let mut files = file_metadata_map(files);
  let mut files_to_remove = Vec::with_capacity(files.len());

  while files.len() > keep_count {
    if let Some((_time, file_data)) = files.pop_first() {
      files_to_remove.push(file_data.path);
    }
  }

  files_to_remove
}

fn file_metadata_map(files: Vec<PathBuf>) -> BTreeMap<SystemTime, FileData> {
  files.into_iter()
      .map(|path| FileData {
        last_access: path.metadata()
            .and_then(|m| m.accessed())
            .unwrap_or(SystemTime::UNIX_EPOCH),
        path,
      })
      .map(|file_data| (file_data.last_access, file_data))
      .collect::<BTreeMap<SystemTime, FileData>>()
}
