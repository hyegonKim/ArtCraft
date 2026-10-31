use log::info;

use crate::client::router_higgsfield_client::RouterHiggsfieldClient;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::provider_error::ProviderError;
use crate::generate::generate_video::generate_video_response::{GenerateVideoResponse, HiggsfieldVideoResponsePayload};
use crate::generate::generate_video::providers::higgsfield::video_request::HiggsfieldVideoRequest;

/// A ready-to-send Higgsfield video request: every keyframe and reference
/// has been uploaded and is referenced by its Higgsfield media id.
#[derive(Clone, Debug)]
pub struct HiggsfieldVideoRequestState {
  pub request: HiggsfieldVideoRequest,
}

impl HiggsfieldVideoRequestState {
  pub async fn send(&self, client: &RouterHiggsfieldClient) -> Result<GenerateVideoResponse, ArtcraftRouterError> {
    info!(
      "Enqueuing Higgsfield {} video job: batch={}, references={}",
      self.request.model_label(), self.request.batch_size(), self.request.medias().len(),
    );

    let response = self.request.send(&client.session).await
        .map_err(|err| ArtcraftRouterError::from(ProviderError::Higgsfield(err)))?;

    let job_ids: Vec<String> = response.job_ids().into_iter().map(|id| id.into_string()).collect();
    let job_set_id = response.first_job_set()
        .map(|job_set| job_set.id.to_string())
        .filter(|_| !job_ids.is_empty())
        .ok_or_else(|| ArtcraftRouterError::ProviderResponseInvalid(
          "Higgsfield accepted the request but returned no jobs".to_string(),
        ))?;

    info!("Higgsfield enqueued job set {} with {} job(s)", job_set_id, job_ids.len());

    Ok(GenerateVideoResponse::Higgsfield(HiggsfieldVideoResponsePayload { job_set_id, job_ids }))
  }
}
