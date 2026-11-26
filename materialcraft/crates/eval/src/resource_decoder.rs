use materialcraft_image::Image;

/// Decodes an image resource by name within the current package
pub type ResourceDecoder<'a> = dyn Fn(&str) -> Option<Image> + Sync + 'a;
