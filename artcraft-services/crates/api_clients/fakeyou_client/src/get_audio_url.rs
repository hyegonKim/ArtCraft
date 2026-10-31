
/// Return the public-facing URL for an audio resource returned by the API
pub fn get_audio_url(rooted_bucket_path: &str, is_production: bool) -> String {
  let bucket = if is_production {
    "vocodes-public"
  } else {
    "dev-vocodes-public"
  };

  format!("https://storage.googleapis.com/{}{}", bucket, rooted_bucket_path)
}
