//! One planned Higgsfield image request, whichever model it's for. The
//! per-model `build.rs` files produce these; the shared draft / request /
//! cost states carry them.

use higgsfield_client::endpoints::generate::image::gpt_image_2::GptImage2Request;
use higgsfield_client::endpoints::generate::image::nano_banana_2::NanoBanana2Request;
use higgsfield_client::endpoints::generate::image::nano_banana_2_lite::NanoBanana2LiteRequest;
use higgsfield_client::endpoints::generate::image::nano_banana_pro::NanoBananaProRequest;
use higgsfield_client::endpoints::generate::image::seedream_4p5::Seedream4p5Request;
use higgsfield_client::endpoints::generate::image::seedream_5p0_lite::Seedream5p0LiteRequest;
use higgsfield_client::endpoints::generate::image::seedream_5p0_pro::Seedream5p0ProRequest;
use higgsfield_client::error::higgsfield_error::HiggsfieldError;
use higgsfield_client::session::higgsfield_session::HiggsfieldSession;
use higgsfield_client::types::enqueue_jobs_response::EnqueueJobsResponse;
use higgsfield_client::types::image_batch_size::ImageBatchSize;
use higgsfield_client::types::media_input::MediaInput;

#[derive(Clone, Debug)]
pub enum HiggsfieldImageRequest {
  NanoBananaPro(NanoBananaProRequest),
  NanoBanana2(NanoBanana2Request),
  NanoBanana2Lite(NanoBanana2LiteRequest),
  GptImage2(GptImage2Request),
  Seedream5p0Pro(Seedream5p0ProRequest),
  Seedream5p0Lite(Seedream5p0LiteRequest),
  Seedream4p5(Seedream4p5Request),
}

impl HiggsfieldImageRequest {
  /// Enqueue on the session (which mints the bearer token and retries once
  /// on a rejected token).
  pub async fn send(&self, session: &HiggsfieldSession) -> Result<EnqueueJobsResponse, HiggsfieldError> {
    match self {
      Self::NanoBananaPro(request) => session.nano_banana_pro(request.clone()).await,
      Self::NanoBanana2(request) => session.nano_banana_2(request.clone()).await,
      Self::NanoBanana2Lite(request) => session.nano_banana_2_lite(request.clone()).await,
      Self::GptImage2(request) => session.gpt_image_2(request.clone()).await,
      Self::Seedream5p0Pro(request) => session.seedream_5p0_pro(request.clone()).await,
      Self::Seedream5p0Lite(request) => session.seedream_5p0_lite(request.clone()).await,
      Self::Seedream4p5(request) => session.seedream_4p5(request.clone()).await,
    }
  }

  /// Attach uploaded reference images (image-to-image).
  pub fn with_reference_images(self, reference_images: Vec<MediaInput>) -> Self {
    match self {
      Self::NanoBananaPro(request) => Self::NanoBananaPro(request.with_reference_images(reference_images)),
      Self::NanoBanana2(request) => Self::NanoBanana2(request.with_reference_images(reference_images)),
      Self::NanoBanana2Lite(request) => Self::NanoBanana2Lite(request.with_reference_images(reference_images)),
      Self::GptImage2(request) => Self::GptImage2(request.with_reference_images(reference_images)),
      Self::Seedream5p0Pro(request) => Self::Seedream5p0Pro(request.with_reference_images(reference_images)),
      Self::Seedream5p0Lite(request) => Self::Seedream5p0Lite(request.with_reference_images(reference_images)),
      Self::Seedream4p5(request) => Self::Seedream4p5(request.with_reference_images(reference_images)),
    }
  }

  pub fn prompt(&self) -> &str {
    match self {
      Self::NanoBananaPro(request) => &request.prompt,
      Self::NanoBanana2(request) => &request.prompt,
      Self::NanoBanana2Lite(request) => &request.prompt,
      Self::GptImage2(request) => &request.prompt,
      Self::Seedream5p0Pro(request) => &request.prompt,
      Self::Seedream5p0Lite(request) => &request.prompt,
      Self::Seedream4p5(request) => &request.prompt,
    }
  }

  pub fn batch_size(&self) -> ImageBatchSize {
    match self {
      Self::NanoBananaPro(request) => request.batch_size,
      Self::NanoBanana2(request) => request.batch_size,
      Self::NanoBanana2Lite(request) => request.batch_size,
      Self::GptImage2(request) => request.batch_size,
      Self::Seedream5p0Pro(request) => request.batch_size,
      Self::Seedream5p0Lite(request) => request.batch_size,
      Self::Seedream4p5(request) => request.batch_size,
    }
  }

  pub fn reference_images(&self) -> &[MediaInput] {
    match self {
      Self::NanoBananaPro(request) => &request.reference_images,
      Self::NanoBanana2(request) => &request.reference_images,
      Self::NanoBanana2Lite(request) => &request.reference_images,
      Self::GptImage2(request) => &request.reference_images,
      Self::Seedream5p0Pro(request) => &request.reference_images,
      Self::Seedream5p0Lite(request) => &request.reference_images,
      Self::Seedream4p5(request) => &request.reference_images,
    }
  }

  /// A short label for logs.
  pub fn model_label(&self) -> &'static str {
    match self {
      Self::NanoBananaPro(_) => "Nano Banana Pro",
      Self::NanoBanana2(_) => "Nano Banana 2",
      Self::NanoBanana2Lite(_) => "Nano Banana 2 Lite",
      Self::GptImage2(_) => "GPT Image 2",
      Self::Seedream5p0Pro(_) => "Seedream 5.0 Pro",
      Self::Seedream5p0Lite(_) => "Seedream 5 Lite",
      Self::Seedream4p5(_) => "Seedream 4.5",
    }
  }
}
