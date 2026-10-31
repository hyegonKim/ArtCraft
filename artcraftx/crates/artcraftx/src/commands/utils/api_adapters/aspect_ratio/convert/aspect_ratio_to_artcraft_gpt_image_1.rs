use crate::commands::utils::api_adapters::aspect_ratio::common_aspect_ratio::CommonAspectRatio;

pub fn aspect_ratio_to_artcraft_gpt_image_1(_aspect_ratio: CommonAspectRatio) -> Option<String> {
  // TODO: We still make calls to two distinct endpoints for image-to-image and text-to-image.
  //  These need to be consolidated to benefit from this approach.
  None
}
