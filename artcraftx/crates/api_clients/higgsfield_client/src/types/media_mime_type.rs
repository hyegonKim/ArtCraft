//! The file types the web app's upload pickers accept.

use crate::types::string_enum::string_enum;

string_enum! {
  /// A reference file's MIME type, as sent to the presign endpoints and as
  /// the `Content-Type` of the storage upload.
  ///
  /// `/fnf/media/batch` accepts exactly the image types (its 422 lists
  /// jpeg, jpg, png, webp, gif, heic, heif, avif); the web app's media
  /// library picker also offers the video and audio types below.
  MediaMimeType {
    ImageJpeg => "image/jpeg",
    ImagePng => "image/png",
    ImageWebp => "image/webp",
    ImageHeic => "image/heic",
    ImageHeif => "image/heif",
    ImageGif => "image/gif",
    ImageAvif => "image/avif",
    VideoMp4 => "video/mp4",
    VideoWebm => "video/webm",
    VideoOgg => "video/ogg",
    VideoQuicktime => "video/quicktime",
    AudioWav => "audio/wav",
    /// What the audio presign hands back for a `wav`.
    AudioXWav => "audio/x-wav",
    AudioMpeg => "audio/mpeg",
    AudioMp4 => "audio/mp4",
    AudioAac => "audio/aac",
    AudioOgg => "audio/ogg",
    AudioWebm => "audio/webm",
  }
}

impl MediaMimeType {
  /// Guess from a file name / extension (case-insensitive). `None` for
  /// anything the web app wouldn't accept.
  pub fn from_file_name(file_name: &str) -> Option<Self> {
    let extension = file_name.rsplit('.').next()?.to_ascii_lowercase();
    let mime = match extension.as_str() {
      "jpg" | "jpeg" => Self::ImageJpeg,
      "png" => Self::ImagePng,
      "webp" => Self::ImageWebp,
      "heic" => Self::ImageHeic,
      "heif" => Self::ImageHeif,
      "gif" => Self::ImageGif,
      "avif" => Self::ImageAvif,
      "mp4" => Self::VideoMp4,
      "webm" => Self::VideoWebm,
      "ogv" => Self::VideoOgg,
      "mov" => Self::VideoQuicktime,
      "wav" => Self::AudioWav,
      "mp3" => Self::AudioMpeg,
      "m4a" => Self::AudioMp4,
      "aac" => Self::AudioAac,
      "ogg" | "oga" => Self::AudioOgg,
      _ => return None,
    };
    Some(mime)
  }

  /// The extension the CDN URL will carry for this type.
  pub fn file_extension(&self) -> &str {
    match self {
      Self::ImageJpeg => "jpg",
      Self::ImagePng => "png",
      Self::ImageWebp => "webp",
      Self::ImageHeic => "heic",
      Self::ImageHeif => "heif",
      Self::ImageGif => "gif",
      Self::ImageAvif => "avif",
      Self::VideoMp4 => "mp4",
      Self::VideoWebm => "webm",
      Self::VideoOgg => "ogv",
      Self::VideoQuicktime => "mov",
      Self::AudioWav | Self::AudioXWav => "wav",
      Self::AudioMpeg => "mp3",
      Self::AudioMp4 => "m4a",
      Self::AudioAac => "aac",
      Self::AudioOgg => "ogg",
      Self::AudioWebm => "webm",
      Self::Other(raw) => raw.rsplit('/').next().unwrap_or(raw),
    }
  }

  pub fn is_image(&self) -> bool {
    self.as_str().starts_with("image/")
  }

  pub fn is_video(&self) -> bool {
    self.as_str().starts_with("video/")
  }

  pub fn is_audio(&self) -> bool {
    self.as_str().starts_with("audio/")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn guesses_from_file_names() {
    assert_eq!(MediaMimeType::from_file_name("shiba.PNG"), Some(MediaMimeType::ImagePng));
    assert_eq!(MediaMimeType::from_file_name("/tmp/a.b/clip.mov"), Some(MediaMimeType::VideoQuicktime));
    assert_eq!(MediaMimeType::from_file_name("tone.wav"), Some(MediaMimeType::AudioWav));
    assert_eq!(MediaMimeType::from_file_name("readme.txt"), None);
    assert_eq!(MediaMimeType::from_file_name("noextension"), None);
  }

  #[test]
  fn classifies_by_family() {
    assert!(MediaMimeType::ImageWebp.is_image());
    assert!(MediaMimeType::VideoMp4.is_video());
    assert!(MediaMimeType::AudioMpeg.is_audio());
    assert!(!MediaMimeType::AudioMpeg.is_image());
    assert!(MediaMimeType::Other("image/gif".to_string()).is_image());
  }

  #[test]
  fn wire_strings_round_trip() {
    for mime in MediaMimeType::known_variants() {
      assert_eq!(&MediaMimeType::from_str_lossy(mime.as_str()), mime);
      assert_eq!(serde_json::to_string(mime).unwrap(), format!("\"{}\"", mime.as_str()));
    }
    assert_eq!(MediaMimeType::VideoMp4.file_extension(), "mp4");
    assert_eq!(MediaMimeType::Other("image/gif".to_string()).file_extension(), "gif");
  }
}
