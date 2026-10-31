use crate::api::image_list_ref::ImageListRef;
use crate::api::image_ref::ImageRef;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::client_error::ClientError;
use crate::generate::generate_splat::generate_splat_request::GenerateSplatRequest;
use crate::generate::generate_splat::splat_generation_plan::SplatGenerationPlan;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

#[derive(Debug, Clone)]
pub struct PlanArtcraftMarble0p1Mini {
  pub prompt: Option<String>,
  pub reference_image: Option<MediaFileToken>,
  pub idempotency_token: String,
}

pub fn plan_generate_splat_artcraft_marble_0p1_mini(
  request: &GenerateSplatRequest,
) -> Result<SplatGenerationPlan, ArtcraftRouterError> {
  let reference_image = resolve_single_image_ref(request.reference_images.clone())?;

  Ok(SplatGenerationPlan::ArtcraftMarble0p1Mini(PlanArtcraftMarble0p1Mini {
    prompt: request.prompt.clone(),
    reference_image,
    idempotency_token: request.get_or_generate_idempotency_token(),
  }))
}

fn resolve_single_image_ref(
  image_list_ref: Option<ImageListRef>,
) -> Result<Option<MediaFileToken>, ArtcraftRouterError> {
  match image_list_ref {
    None => Ok(None),
    Some(ImageListRef::MediaFileTokens(tokens)) => {
      Ok(tokens.into_iter().next())
    }
    Some(ImageListRef::Urls(_)) => {
      Err(ArtcraftRouterError::Client(ClientError::ArtcraftOnlySupportsMediaTokens))
    }
    Some(ImageListRef::Sources(refs)) => {
      let mut tokens = Vec::with_capacity(refs.len());
      for image_ref in refs {
        match image_ref {
          ImageRef::MediaFileToken(token) => tokens.push(token),
          _ => return Err(ArtcraftRouterError::Client(ClientError::ArtcraftOnlySupportsMediaTokens)),
        }
      }
      Ok(tokens.into_iter().next())
    }
  }
}
