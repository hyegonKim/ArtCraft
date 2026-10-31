//! A reference file with its role, as listed in a request's `medias`.

use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::types::media_input::MediaInput;
use crate::types::media_role::MediaRole;
use serde::{Deserialize, Serialize};

/// One entry of a `medias` list: `{"role": "...", "data": {<MediaInput>}}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaReference {
  pub role: MediaRole,
  pub data: MediaInput,
}

impl MediaReference {
  pub fn new(role: MediaRole, data: MediaInput) -> Self {
    Self { role, data }
  }

  /// A style / subject reference image.
  pub fn image(data: MediaInput) -> Self {
    Self::new(MediaRole::Image, data)
  }

  /// The video's first frame.
  pub fn start_frame(data: MediaInput) -> Self {
    Self::new(MediaRole::StartImage, data)
  }

  /// The video's last frame.
  pub fn end_frame(data: MediaInput) -> Self {
    Self::new(MediaRole::EndImage, data)
  }

  /// A reference video clip.
  pub fn video(data: MediaInput) -> Self {
    Self::new(MediaRole::Video, data)
  }

  /// A reference audio track.
  pub fn audio(data: MediaInput) -> Self {
    Self::new(MediaRole::Audio, data)
  }
}

/// Reject roles a model doesn't take, and duplicate frames (a video has one
/// start and one end). `model` names the model in the error message.
pub fn validate_media_roles(medias: &[MediaReference], allowed: &[MediaRole], model: &str) -> Result<(), HiggsfieldClientError> {
  for reference in medias {
    if !allowed.contains(&reference.role) {
      let allowed_list: Vec<&str> = allowed.iter().map(|role| role.as_str()).collect();
      return Err(HiggsfieldClientError::InvalidRequest(format!(
        "{model} does not take `{}` media; it accepts: {}",
        reference.role, allowed_list.join(", "),
      )));
    }
  }

  for role in [MediaRole::StartImage, MediaRole::EndImage] {
    let count = medias.iter().filter(|reference| reference.role == role).count();
    if count > 1 {
      return Err(HiggsfieldClientError::InvalidRequest(format!("{model} takes at most one `{role}`; got {count}")));
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::{json, Value};

  fn input(id: &str) -> MediaInput {
    MediaInput::uploaded(id, format!("https://cdn.example.com/user_x/{id}.png"))
  }

  #[test]
  fn serializes_like_the_web_app() {
    let actual: Value = serde_json::to_value(MediaReference::start_frame(input("m1"))).unwrap();
    assert_eq!(actual, json!({"role": "start_image", "data": {"id": "m1", "type": "media_input", "url": "https://cdn.example.com/user_x/m1.png"}}));
  }

  #[test]
  fn parses_the_server_echo() {
    let echoed: Vec<MediaReference> = serde_json::from_str(r#"[{"role":"image","data":{"id":"m1","type":"media_input","url":"https://cdn.example.com/user_x/m1.png"}}]"#).unwrap();
    assert_eq!(echoed, vec![MediaReference::image(input("m1"))]);
  }

  #[test]
  fn role_validation() {
    let allowed = [MediaRole::StartImage, MediaRole::Image];
    assert!(validate_media_roles(&[], &allowed, "m").is_ok());
    assert!(validate_media_roles(&[MediaReference::start_frame(input("a")), MediaReference::image(input("b")), MediaReference::image(input("c"))], &allowed, "m").is_ok());

    let err = validate_media_roles(&[MediaReference::end_frame(input("a"))], &allowed, "Grok").unwrap_err();
    assert!(err.to_string().contains("Grok does not take `end_image`"), "{err}");

    let err = validate_media_roles(&[MediaReference::start_frame(input("a")), MediaReference::start_frame(input("b"))], &allowed, "m").unwrap_err();
    assert!(err.to_string().contains("at most one `start_image`"), "{err}");
  }
}
