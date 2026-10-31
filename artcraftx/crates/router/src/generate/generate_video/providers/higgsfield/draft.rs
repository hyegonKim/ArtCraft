use higgsfield_client::endpoints::generate::video::seedance_2p5::Seedance2p5Resolution;
use higgsfield_client::endpoints::generate::video::seedance_2p5_edit::Seedance2p5EditRequest;
use higgsfield_client::types::image_batch_size::ImageBatchSize;
use higgsfield_client::types::media_reference::MediaReference;
use higgsfield_client::types::video_bitrate_mode::VideoBitrateMode;
use log::{info, warn};

use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_video::providers::higgsfield::common::HiggsfieldVideoReferences;
use crate::generate::generate_video::providers::higgsfield::request::HiggsfieldVideoRequestState;
use crate::generate::generate_video::providers::higgsfield::video_request::HiggsfieldVideoRequest;
use crate::generate::generate_video::video_generation_draft_context::VideoGenerationDraftContext;
use crate::utils::higgsfield_media::{upload_image_ref, upload_media_list, HiggsfieldMediaKind, HiggsfieldUploadContext};

/// What a `build.rs` planned, before media is uploaded.
#[derive(Clone, Debug)]
pub enum HiggsfieldVideoPlan {
  /// A request that only needs its references attached.
  Request(HiggsfieldVideoRequest),
  /// Seedance 2.5 Edit can't be constructed until its source clip has a
  /// Higgsfield media id.
  Seedance2p5Edit(Seedance2p5EditPlan),
}

#[derive(Clone, Debug)]
pub struct Seedance2p5EditPlan {
  pub prompt: String,
  pub resolution: Seedance2p5Resolution,
  pub batch_size: ImageBatchSize,
  pub generate_audio: bool,
  pub bitrate_mode: VideoBitrateMode,
}

impl HiggsfieldVideoPlan {
  pub fn model_label(&self) -> &'static str {
    match self {
      Self::Request(request) => request.model_label(),
      Self::Seedance2p5Edit(_) => "Seedance 2.5 Edit",
    }
  }

  pub fn batch_size(&self) -> u32 {
    match self {
      Self::Request(request) => request.batch_size(),
      Self::Seedance2p5Edit(plan) => plan.batch_size.as_u32(),
    }
  }
}

/// A planned Higgsfield video request whose keyframes / references still
/// have to be uploaded. `to_request` does the uploads, tags each by role,
/// and produces the sendable state.
#[derive(Clone, Debug)]
pub struct HiggsfieldVideoDraftState {
  pub plan: HiggsfieldVideoPlan,
  pub unhandled_request_state: Option<HiggsfieldVideoReferences>,
  /// Run Higgsfield's IP check on uploaded images and clips (the Seedance
  /// models refuse media that hasn't been checked).
  pub ip_check: bool,
}

impl HiggsfieldVideoDraftState {
  pub async fn to_request(
    &mut self,
    draft_context: &VideoGenerationDraftContext<'_>,
  ) -> Result<HiggsfieldVideoRequestState, ArtcraftRouterError> {
    let client = draft_context.get_higgsfield_client_ref()?;
    let uploads = HiggsfieldUploadContext {
      session: &client.session,
      maybe_map: draft_context.media_file_to_artcraft_url_map,
      maybe_cache: draft_context.asset_upload_cache,
      maybe_observer: draft_context.asset_upload_observer,
    };
    let label = self.plan.model_label();

    let references = self.unhandled_request_state.take().unwrap_or_default();

    let start_frame = upload_image_ref(uploads, references.start_frame, self.ip_check).await?;
    let end_frame = upload_image_ref(uploads, references.end_frame, self.ip_check).await?;
    let images = upload_media_list(uploads, references.reference_images.map(Into::into), HiggsfieldMediaKind::Image, self.ip_check).await?;
    let mut videos = upload_media_list(uploads, references.reference_videos.map(Into::into), HiggsfieldMediaKind::Video, self.ip_check).await?;
    // Audio has no IP check.
    let audio = upload_media_list(uploads, references.reference_audio.map(Into::into), HiggsfieldMediaKind::Audio, false).await?;

    let mut request = match self.plan.clone() {
      HiggsfieldVideoPlan::Request(request) => request,
      HiggsfieldVideoPlan::Seedance2p5Edit(plan) => {
        if videos.is_empty() {
          return Err(ArtcraftRouterError::InvalidInput("Seedance 2.5 Edit needs the video to edit as a reference".to_string()));
        }
        let source_video = videos.remove(0);
        if !videos.is_empty() {
          warn!("Seedance 2.5 Edit edits one clip; ignoring {} extra reference video(s)", videos.len());
          videos.clear();
        }
        let mut request = Seedance2p5EditRequest::new(plan.prompt, source_video, plan.resolution);
        request.batch_size = plan.batch_size;
        request.generate_audio = plan.generate_audio;
        request.bitrate_mode = plan.bitrate_mode;
        HiggsfieldVideoRequest::Seedance2p5Edit(request)
      }
    };

    attach(&mut request, start_frame.map(MediaReference::start_frame));
    attach(&mut request, end_frame.map(MediaReference::end_frame));
    for image in images {
      request.push_media(MediaReference::image(image));
    }
    for video in videos {
      request.push_media(MediaReference::video(video));
    }
    for clip in audio {
      request.push_media(MediaReference::audio(clip));
    }

    info!("Higgsfield {} request finalized with {} reference(s)", label, request.medias().len());
    Ok(HiggsfieldVideoRequestState { request })
  }
}

fn attach(request: &mut HiggsfieldVideoRequest, reference: Option<MediaReference>) {
  if let Some(reference) = reference {
    request.push_media(reference);
  }
}
