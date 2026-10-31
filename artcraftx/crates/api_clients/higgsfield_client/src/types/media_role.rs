//! What a reference file is *for* in a generation request.

use crate::types::string_enum::string_enum;

string_enum! {
  /// The `role` of an entry in a request's `medias` list. The set comes
  /// from the web app's own filter (`role === "image" || "start_image" ||
  /// "end_image" || "video" || "audio"`); which roles a given model accepts
  /// is per model — see each request type's `MEDIA_ROLES`.
  MediaRole {
    /// A style / subject reference image (image-to-image, or a video
    /// model's "references").
    Image => "image",

    /// The first frame of a video.
    StartImage => "start_image",

    /// The last frame of a video.
    EndImage => "end_image",

    /// A reference video clip (Seedance 2.x, MiniMax H3 references).
    Video => "video",

    /// A reference audio track (Seedance 2.x, MiniMax H3 references).
    Audio => "audio",
  }
}

impl MediaRole {
  /// Frames are singular: at most one start and one end image per request.
  pub fn is_frame(&self) -> bool {
    matches!(self, Self::StartImage | Self::EndImage)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wire_strings_match_the_web_app() {
    let wire: Vec<&str> = MediaRole::known_variants().iter().map(|r| r.as_str()).collect();
    assert_eq!(wire, ["image", "start_image", "end_image", "video", "audio"]);
    assert!(MediaRole::StartImage.is_frame());
    assert!(!MediaRole::Image.is_frame());
  }
}
