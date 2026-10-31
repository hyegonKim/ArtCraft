use grok_consumer_client::endpoint_bindings::generate_image_websocket::messages::websocket_client_message::{FastAspectRatio, QualityAspectRatio};
use grok_consumer_client::prompt_flags::PromptFlags;

use crate::api::router_aspect_ratio::RouterAspectRatio;
use crate::client::router_grok_client::RouterGrokClient;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::provider_error::ProviderError;
use crate::generate::generate_image::generate_image_response::{
  GenerateImageResponse, GrokImageResponsePayload,
};

/// A ready-to-send first-party Grok Imagine image request. Fast vs quality
/// ("pro") is `enable_pro`; the two modes support slightly different aspect
/// ratios (see [`fast_aspect`] / [`quality_aspect`]).
#[derive(Debug, Clone)]
pub struct GrokImagineImageRequestState {
  pub prompt: String,
  pub aspect_ratio: RouterAspectRatio,
  pub enable_pro: bool,
}

impl GrokImagineImageRequestState {
  /// Send the prompt on the client's imagine websocket and return the prompt's
  /// request id right away. Finished images arrive later on the same socket,
  /// tagged with that id — the caller's polling thread collects them.
  pub async fn send(
    &self,
    client: &RouterGrokClient,
  ) -> Result<GenerateImageResponse, ArtcraftRouterError> {
    let websocket = &client.image_websocket;
    let flags = PromptFlags::default();

    let request_id = if self.enable_pro {
      websocket
          .send_quality_image_prompt_with_retry(&self.prompt, quality_aspect(self.aspect_ratio), &flags)
          .await
    } else {
      websocket
          .send_fast_image_prompt_with_retry(&self.prompt, fast_aspect(self.aspect_ratio), &flags)
          .await
    }
    .map_err(|err| grok_err("send prompt", err))?;

    Ok(GenerateImageResponse::Grok(GrokImageResponsePayload {
      request_id: request_id.to_string(),
    }))
  }
}

fn grok_err(stage: &str, err: impl std::fmt::Display) -> ArtcraftRouterError {
  ArtcraftRouterError::Provider(ProviderError::Grok(format!("{stage}: {err}")))
}

/// Map a router aspect ratio to fast mode's supported set (nearest fit).
fn fast_aspect(ratio: RouterAspectRatio) -> FastAspectRatio {
  match ratio {
    RouterAspectRatio::WideThreeByTwo | RouterAspectRatio::WideFourByThree => FastAspectRatio::WideThreeByTwo,
    RouterAspectRatio::WideSixteenByNine
    | RouterAspectRatio::Wide
    | RouterAspectRatio::WideTwentyOneByNine => FastAspectRatio::WideSixteenByNine,
    RouterAspectRatio::TallTwoByThree
    | RouterAspectRatio::TallThreeByFour
    | RouterAspectRatio::TallFourByFive => FastAspectRatio::TallTwoByThree,
    RouterAspectRatio::TallNineBySixteen
    | RouterAspectRatio::Tall
    | RouterAspectRatio::TallNineByTwentyOne => FastAspectRatio::TallNineBySixteen,
    _ => FastAspectRatio::Square,
  }
}

/// Map a router aspect ratio to quality mode's larger supported set.
fn quality_aspect(ratio: RouterAspectRatio) -> QualityAspectRatio {
  match ratio {
    RouterAspectRatio::WideThreeByTwo => QualityAspectRatio::WideThreeByTwo,
    RouterAspectRatio::WideFourByThree => QualityAspectRatio::WideFourByThree,
    RouterAspectRatio::WideSixteenByNine | RouterAspectRatio::Wide => QualityAspectRatio::WideSixteenByNine,
    RouterAspectRatio::WideTwentyOneByNine => QualityAspectRatio::WideTwentyOneByNine,
    RouterAspectRatio::TallTwoByThree
    | RouterAspectRatio::TallThreeByFour
    | RouterAspectRatio::TallFourByFive => QualityAspectRatio::TallTwoByThree,
    RouterAspectRatio::TallNineBySixteen
    | RouterAspectRatio::Tall
    | RouterAspectRatio::TallNineByTwentyOne => QualityAspectRatio::TallNineBySixteen,
    _ => QualityAspectRatio::Square,
  }
}
