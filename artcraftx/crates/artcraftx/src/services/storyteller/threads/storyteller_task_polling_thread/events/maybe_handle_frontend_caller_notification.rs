use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::functional_events::gaussian_generation_complete_event::{GaussianGenerationCompleteEvent, GeneratedGaussian};
use crate::events::functional_events::object_generation_complete_event::{GeneratedObject, ObjectGenerationCompleteEvent};
use crate::events::functional_events::text_to_image_generation_complete_event::{GeneratedImage, TextToImageGenerationCompleteEvent};
use crate::events::functional_events::video_generation_complete_event::{GeneratedVideo, VideoGenerationCompleteEvent};
use artcraft_client::api_defs::jobs::get_job_status::JobStatusPayload;
use artcraft_client::api_defs::media_file::list_media_files_by_job::JobMediaFileInfo;
use artcraft_client::api_defs::utils::media_links_to_thumbnail_template::media_links_to_thumbnail_template;
use errors::AnyhowResult;
use log::warn;
use sqlite_database::queries::task::Task;
use sqlite_identifiers::enums::task_type::TaskType;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use tauri::AppHandle;
use url::Url;

/// One generated output file, in the shape the typed frontend events share.
struct GeneratedFile {
  media_token: MediaFileToken,
  cdn_url: Url,
  maybe_thumbnail_template: Option<String>,
}

pub async fn maybe_handle_frontend_caller_notification(
  app: &AppHandle,
  task: &Task,
  job: &JobStatusPayload,
  media_files: &[JobMediaFileInfo],
) -> AnyhowResult<()> {
  // Every file the job produced. When the media-file -> source-job linkage
  // isn't populated, fall back to the job's single result entity.
  let generated_files = collect_generated_files(job, media_files);

  if generated_files.is_empty() {
    warn!("Job has no result files for task: {:?}", task);
    return Ok(()); // No results, nothing to announce.
  }

  match task.task_type {
    TaskType::ImageGeneration => {
      let event = TextToImageGenerationCompleteEvent {
        generated_images: generated_files.into_iter()
            .map(|file| GeneratedImage {
              media_token: file.media_token,
              cdn_url: file.cdn_url,
              maybe_thumbnail_template: file.maybe_thumbnail_template,
            })
            .collect(),
        maybe_frontend_subscriber_id: task.frontend_subscriber_id.clone(),
        maybe_frontend_subscriber_payload: task.frontend_subscriber_payload.clone(),
      };
      event.send_infallible(app);
    }
    TaskType::VideoGeneration => {
      let event = VideoGenerationCompleteEvent {
        generated_videos: generated_files.into_iter()
            .map(|file| GeneratedVideo {
              media_token: file.media_token,
              cdn_url: file.cdn_url,
              maybe_thumbnail_template: file.maybe_thumbnail_template,
            })
            .collect(),
        maybe_frontend_subscriber_id: task.frontend_subscriber_id.clone(),
        maybe_frontend_subscriber_payload: task.frontend_subscriber_payload.clone(),
      };
      event.send_infallible(app);
    }
    TaskType::MeshGeneration => {
      let event = ObjectGenerationCompleteEvent {
        generated_objects: generated_files.into_iter()
            .map(|file| GeneratedObject {
              media_token: file.media_token,
              cdn_url: file.cdn_url,
              maybe_thumbnail_template: file.maybe_thumbnail_template,
            })
            .collect(),
        maybe_frontend_subscriber_id: task.frontend_subscriber_id.clone(),
        maybe_frontend_subscriber_payload: task.frontend_subscriber_payload.clone(),
      };
      event.send_infallible(app);
    }
    TaskType::SplatGeneration => {
      let event = GaussianGenerationCompleteEvent {
        generated_gaussians: generated_files.into_iter()
            .map(|file| GeneratedGaussian {
              media_token: file.media_token,
              cdn_url: file.cdn_url,
              maybe_thumbnail_template: file.maybe_thumbnail_template,
            })
            .collect(),
        maybe_frontend_subscriber_id: task.frontend_subscriber_id.clone(),
        maybe_frontend_subscriber_payload: task.frontend_subscriber_payload.clone(),
      };
      event.send_infallible(app);
    }
    TaskType::AudioGeneration => {
      // No typed audio notification yet; generic completion events still fire.
    }
  }

  Ok(())
}

fn collect_generated_files(
  job: &JobStatusPayload,
  media_files: &[JobMediaFileInfo],
) -> Vec<GeneratedFile> {
  if !media_files.is_empty() {
    return media_files.iter()
        .map(|file| GeneratedFile {
          media_token: file.token.clone(),
          cdn_url: file.media_links.cdn_url.clone(),
          maybe_thumbnail_template: media_links_to_thumbnail_template(&file.media_links)
              .map(|template| template.to_owned()),
        })
        .collect();
  }

  // Fall back to the single result entity on the job itself.
  job.maybe_result
      .as_ref()
      .map(|result| GeneratedFile {
        media_token: MediaFileToken::new_from_str(&result.entity_token),
        cdn_url: result.media_links.cdn_url.clone(),
        maybe_thumbnail_template: media_links_to_thumbnail_template(&result.media_links)
            .map(|template| template.to_owned()),
      })
      .into_iter()
      .collect()
}
