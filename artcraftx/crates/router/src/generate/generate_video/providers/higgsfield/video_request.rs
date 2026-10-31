//! One planned Higgsfield video request, whichever model it's for. The
//! per-model `build.rs` files produce these (or, for Seedance 2.5 Edit, a
//! plan that becomes one once the source clip is uploaded); the shared
//! draft / request / cost states carry them.

use higgsfield_client::endpoints::generate::video::grok_imagine_1p5::GrokImagine1p5Request;
use higgsfield_client::endpoints::generate::video::kling_3p0::Kling3p0Request;
use higgsfield_client::endpoints::generate::video::minimax_h3::MinimaxH3Request;
use higgsfield_client::endpoints::generate::video::seedance_2p0::Seedance2p0Request;
use higgsfield_client::endpoints::generate::video::seedance_2p0_mini::Seedance2p0MiniRequest;
use higgsfield_client::endpoints::generate::video::seedance_2p5::Seedance2p5Request;
use higgsfield_client::endpoints::generate::video::seedance_2p5_edit::Seedance2p5EditRequest;
use higgsfield_client::error::higgsfield_error::HiggsfieldError;
use higgsfield_client::session::higgsfield_session::HiggsfieldSession;
use higgsfield_client::types::enqueue_jobs_response::EnqueueJobsResponse;
use higgsfield_client::types::media_reference::MediaReference;

#[derive(Clone, Debug)]
pub enum HiggsfieldVideoRequest {
  Seedance2p5(Seedance2p5Request),
  Seedance2p5Edit(Seedance2p5EditRequest),
  Seedance2p0(Seedance2p0Request),
  Seedance2p0Mini(Seedance2p0MiniRequest),
  MinimaxH3(MinimaxH3Request),
  /// Both Kling 3.0 model ids: the "mode" (std / pro / 4K) is in the request.
  Kling3p0(Kling3p0Request),
  GrokImagine1p5(GrokImagine1p5Request),
}

impl HiggsfieldVideoRequest {
  /// Enqueue on the session (which mints the bearer token and retries once
  /// on a rejected token).
  pub async fn send(&self, session: &HiggsfieldSession) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    match self {
      Self::Seedance2p5(request) => session.seedance_2p5(request.clone()).await,
      Self::Seedance2p5Edit(request) => session.seedance_2p5_edit(request.clone()).await,
      Self::Seedance2p0(request) => session.seedance_2p0(request.clone()).await,
      Self::Seedance2p0Mini(request) => session.seedance_2p0_mini(request.clone()).await,
      Self::MinimaxH3(request) => session.minimax_h3(request.clone()).await,
      Self::Kling3p0(request) => session.kling_3p0(request.clone()).await,
      Self::GrokImagine1p5(request) => session.grok_imagine_1p5(request.clone()).await,
    }
  }

  /// Attach an uploaded reference with its role. (For Seedance 2.5 Edit the
  /// source clip is set at construction; this adds the extra references.)
  pub fn push_media(&mut self, reference: MediaReference) {
    match self {
      Self::Seedance2p5(request) => request.medias.push(reference),
      Self::Seedance2p5Edit(request) => request.references.push(reference),
      Self::Seedance2p0(request) => request.medias.push(reference),
      Self::Seedance2p0Mini(request) => request.medias.push(reference),
      Self::MinimaxH3(request) => request.medias.push(reference),
      Self::Kling3p0(request) => request.medias.push(reference),
      Self::GrokImagine1p5(request) => request.medias.push(reference),
    }
  }

  /// The attached references (excluding Seedance 2.5 Edit's source clip).
  pub fn medias(&self) -> &[MediaReference] {
    match self {
      Self::Seedance2p5(request) => &request.medias,
      Self::Seedance2p5Edit(request) => &request.references,
      Self::Seedance2p0(request) => &request.medias,
      Self::Seedance2p0Mini(request) => &request.medias,
      Self::MinimaxH3(request) => &request.medias,
      Self::Kling3p0(request) => &request.medias,
      Self::GrokImagine1p5(request) => &request.medias,
    }
  }

  pub fn prompt(&self) -> &str {
    match self {
      Self::Seedance2p5(request) => &request.prompt,
      Self::Seedance2p5Edit(request) => &request.prompt,
      Self::Seedance2p0(request) => &request.prompt,
      Self::Seedance2p0Mini(request) => &request.prompt,
      Self::MinimaxH3(request) => &request.prompt,
      Self::Kling3p0(request) => &request.prompt,
      Self::GrokImagine1p5(request) => &request.prompt,
    }
  }

  /// How many clips the request renders.
  pub fn batch_size(&self) -> u32 {
    match self {
      Self::Seedance2p5(request) => request.batch_size.as_u32(),
      Self::Seedance2p5Edit(request) => request.batch_size.as_u32(),
      Self::Seedance2p0(request) => request.batch_size.as_u32(),
      Self::Seedance2p0Mini(request) => request.batch_size.as_u32(),
      Self::MinimaxH3(_) | Self::Kling3p0(_) | Self::GrokImagine1p5(_) => 1,
    }
  }

  /// A short label for logs.
  pub fn model_label(&self) -> &'static str {
    match self {
      Self::Seedance2p5(_) => "Seedance 2.5",
      Self::Seedance2p5Edit(_) => "Seedance 2.5 Edit",
      Self::Seedance2p0(_) => "Seedance 2.0",
      Self::Seedance2p0Mini(_) => "Seedance 2.0 Mini",
      Self::MinimaxH3(_) => "MiniMax H3",
      Self::Kling3p0(_) => "Kling 3.0",
      Self::GrokImagine1p5(_) => "Grok Imagine 1.5",
    }
  }
}
