//! Probably should be in its own "static_config" crate.

use url::{Host, Url};

/// Reports if the URL is inappropriate for downloading as a video (W2L, etc)
pub fn is_bad_video_download_url(url: &str) -> anyhow::Result<bool> {

  if url.trim().is_empty() {
    // We can't use empty URLs.
    return Ok(true);
  }

  let url = Url::parse(url)?;

  match url.host() {
    Some(Host::Domain(domain)) => {
      let domain = domain.to_lowercase();

      // TODO: If this list grows, store it in a static hashset
      let bad_host = domain.contains("tiktok.com")
          || domain.contains("vm.tiktok.com"); // NB: This hostname is known for never disconnecting, which freezes the job

      if bad_host {
        return Ok(true)
      }
    }
    _ => {},
  }

  Ok(false)
}

#[cfg(test)]
mod tests {
  use crate::is_bad_video_download_url::is_bad_video_download_url;

  #[test]
  fn bad_download_url() {
    assert_eq!(is_bad_video_download_url("").unwrap(), true);
    assert_eq!(is_bad_video_download_url("   ").unwrap(), true);
    assert_eq!(is_bad_video_download_url("https://vm.tiktok.com/ZMNYjT7Xy/?k=1").unwrap(), true); // NB: We get lots of these
  }

  #[test]
  fn is_good_image_url() {
    assert_eq!(is_bad_video_download_url("https://i.imgur.com/8U3IdUa.png").unwrap(), false);
  }

  #[test]
  fn is_good_video_url() {
    assert_eq!(is_bad_video_download_url("https://y.yarn.co/e8ae7dd1-949c-4b94-b0d4-564125e888d8.mp4").unwrap(), false);
    assert_eq!(is_bad_video_download_url("https://www.cijianlink.com/video/input_5.mp4").unwrap(), false);
    assert_eq!(is_bad_video_download_url("https://replicate.delivery/mgxm/67fe4c5c-f47e-49ae-941d-7531acbf3220/output.mp4").unwrap(), false);
  }
}
