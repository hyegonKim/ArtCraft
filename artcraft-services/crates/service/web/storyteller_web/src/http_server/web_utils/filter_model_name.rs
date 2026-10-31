
pub fn filter_model_name(name: &str) -> String {
  // We're not revealing some of the models we use
  name.replace("sadtalker", "faceanimator")
      .replace("sad-talker", "face-animator")
      .replace("sad_talker", "face_animator")
      .replace("vall-e-x", "voice_designer")
      .replace("vall_e_x", "voice_designer")
      .replace("vallex", "voice_designer")
}

pub fn maybe_filter_model_name(name: Option<&str>) -> Option<String> {
  name.map(|name| filter_model_name(name))
}

#[cfg(test)]
mod tests {
  use crate::http_server::web_utils::filter_model_name::maybe_filter_model_name;

  #[test]
  fn test_maybe_filter_model_name() {
    // Unfiltered
    assert_eq!(maybe_filter_model_name(None), None);
    assert_eq!(maybe_filter_model_name(Some("foobarbaz")), Some("foobarbaz".to_string()));

    // Filter raw field values
    assert_eq!(maybe_filter_model_name(Some("sad_talker")), Some("face_animator".to_string()));

    // Filter inclusion in pod worker names
    assert_eq!(maybe_filter_model_name(Some("inference-job-sadtalker-5df55cfbb7-ngxzh")), Some("inference-job-faceanimator-5df55cfbb7-ngxzh".to_string()));
  }
}
