use crate::events::notification_event::{NotificationEvent, NotificationModelType};
use crate::state::app_dir::AppDataRoot;
use crate::transfer::download::{download_async, ProgressUpdate};
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use log::{debug, error, info};
use ml_weights_registry::weights_registry::weight_descriptor::WeightFunction;
use ml_weights_registry::weights_registry::weights::{CLIP_JSON, DIS_MEDIUM_ONNX, LYKON_DEAMSHAPER_7_TEXT_ENCODER_FP16, LYKON_DEAMSHAPER_7_VAE, SDXL_TURBO_CLIP_TEXT_ENCODER, SDXL_TURBO_CLIP_TEXT_ENCODER_2, SDXL_TURBO_UNET, SDXL_TURBO_VAE, SIMIANLUO_LCM_DREAMSHAPER_V7_UNET};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tempfile::NamedTempFile;
use tokio::task::JoinHandle;
use ml_weights_registry::weights_registry::flux_weights::{FLUX_SCHNELL, FLUX_SCHNELL_AUTOENCODER, GOOGLE_T5_V1_1_XXL_CONFIG, GOOGLE_T5_V1_1_XXL_MODEL, LMZ_CANDLE_FLUX_SCHNELL_QUANTIZED_GGUF, LMZ_T5_TOKENIZER_JSON, OPENAI_CLIP_VIT_P14_MODEL, OPENAI_CLIP_VIT_P14_TOKENIZER_JSON};

const NOTIFICATION_CHANNEL_NAME : &str = "notification";
const MAX_FILES : usize = 8;
const CHUNK_SIZE : usize = 1024 * 1024;
const PARALLEL_FAILURES : usize = 8;
const MAX_RETRIES : usize = 30;

struct ModelDownloadStatusChannel {
  pub model_name: String,
  pub model_type: NotificationModelType,
  pub final_file_path: PathBuf,
  pub receiver: Receiver<ProgressUpdate>,
}

pub async fn downloader_thread(app_data_root: AppDataRoot, app: AppHandle) -> ! {

  let mut status_check_queue : Arc<Mutex<Vec<ModelDownloadStatusChannel>>> = Arc::new(Mutex::new(Vec::new()));

  {
    let status_check_queue2 = status_check_queue.clone();
    let app2 = app.clone();

    tokio::spawn(download_notify_thread(status_check_queue2, app2));
  }

  let mut download_queue = VecDeque::new();

  download_queue.push_back(CLIP_JSON);
  download_queue.push_back(LYKON_DEAMSHAPER_7_TEXT_ENCODER_FP16);
  download_queue.push_back(LYKON_DEAMSHAPER_7_VAE);
  download_queue.push_back(SIMIANLUO_LCM_DREAMSHAPER_V7_UNET);
  download_queue.push_back(SDXL_TURBO_CLIP_TEXT_ENCODER); // TODO(bt): Why is this still needed?
  download_queue.push_back(DIS_MEDIUM_ONNX);
  //download_queue.push_back(SDXL_TURBO_CLIP_TEXT_ENCODER_2); // TODO(bt): REMOVE
  //download_queue.push_back(SDXL_TURBO_VAE); // TODO(bt): REMOVE
  //download_queue.push_back(SDXL_TURBO_UNET); // TODO(bt): REMOVE

  // Flux:
  download_queue.push_back(FLUX_SCHNELL);
  download_queue.push_back(FLUX_SCHNELL_AUTOENCODER);
  download_queue.push_back(LMZ_CANDLE_FLUX_SCHNELL_QUANTIZED_GGUF);
  download_queue.push_back(GOOGLE_T5_V1_1_XXL_MODEL);
  download_queue.push_back(GOOGLE_T5_V1_1_XXL_CONFIG);
  download_queue.push_back(LMZ_T5_TOKENIZER_JSON);
  download_queue.push_back(OPENAI_CLIP_VIT_P14_MODEL);
  download_queue.push_back(OPENAI_CLIP_VIT_P14_TOKENIZER_JSON);

  let mut handles = FuturesUnordered::new();

  while let Some(model) = download_queue.pop_front() {
    let url = model.r2_download_url.to_string();

    let filename = app_data_root.weights_dir().weight_path(&model);

    if filename.exists() {
      info!("File already exists: {:?}", &filename);
      continue;
    }

    let headers = None;
    
    let (tx, rx) = std::sync::mpsc::channel();

    info!("Download: {:?}", filename);

    let temp_file = match app_data_root.temp_dir().new_named_temp_file() {
      Ok(file) => file,
      Err(err) => {
        error!("Couldn't create a temporary file: {:?}", err);
        continue;
      }
    };

    let model_type = match model.function {
      WeightFunction::TextEncoder => NotificationModelType::TextEncoder,
      WeightFunction::TextTokenizer => NotificationModelType::TextTokenizer,
      WeightFunction::Unet => NotificationModelType::Unet,
      WeightFunction::Vae => NotificationModelType::Vae,
      WeightFunction::ImageSegmentation => NotificationModelType::ImageSegmentation,
      WeightFunction::ImageGeneration => NotificationModelType::ImageSegmentation, // TODO FIX
    };

    let result = app.emit(NOTIFICATION_CHANNEL_NAME, NotificationEvent::ModelDownloadStarted {
      model_name: model.name,
      model_type,
    });

    if let Err(err) = result {
      error!("Could not emit event: {}", err);
    }

    let task = tokio::spawn(download_async(
      url,
      temp_file,
      filename.clone(),
      MAX_FILES,
      CHUNK_SIZE,
      PARALLEL_FAILURES,
      MAX_RETRIES,
      headers,
      Some(tx),
    ));

    match status_check_queue.lock() {
      Err(err) => error!("Could not unlock: {:?}", err),
      Ok(mut lock) => {
        lock.push(ModelDownloadStatusChannel {
          receiver: rx,
          final_file_path: filename.clone(),
          model_name: model.to_descriptive_name(),
          model_type,
        })
      }
    }

    handles.push(task);
  }

  while let Some(result) = handles.next().await {
    match result {
      Err(err) => error!("Error downloading: {:?}", err),
      Ok(_) => {},
    }
  }

  loop {
    // TODO(bt,2025-03-10): Queue management for new files entering the queue.
    tokio::time::sleep(std::time::Duration::from_millis(60_000)).await;
  }
}

pub async fn download_notify_thread(queue: Arc<Mutex<Vec<ModelDownloadStatusChannel>>>, app: AppHandle) -> ! {
  loop {
    match queue.lock() {
      Err(err) => error!("Could not unlock: {:?}", err),
      Ok(mut lock) => {
        debug!(">>>>> Notifier has {} items", lock.len());

        let mut reenqueue = Vec::with_capacity(lock.len());

        for item in (*lock).drain(..) {
          if let Ok(update) = item.receiver.try_recv() {
            let result = app.emit(NOTIFICATION_CHANNEL_NAME, NotificationEvent::ModelDownloadProgress {
              model_name: &item.model_name,
              model_type: item.model_type,
              currently_downloaded_bytes: update.complete,
              total_file_bytes: update.total_length,
            });

            if let Err(err) = result {
              error!("Could not emit event: {}", err);
            }
          }

          if item.final_file_path.exists() {
            debug!("No longer sending notifications for {:?}", &item.final_file_path);

            let result = app.emit(NOTIFICATION_CHANNEL_NAME, NotificationEvent::ModelDownloadComplete {
              model_name: &item.model_name,
              model_type: item.model_type,
            });

            if let Err(err) = result {
              error!("Could not emit event: {}", err);
            }
          } else {
            reenqueue.push(item);
          }
        }

        lock.extend(reenqueue);
      }
    }

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
  }
}
