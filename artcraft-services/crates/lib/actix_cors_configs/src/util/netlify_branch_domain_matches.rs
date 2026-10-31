use actix_http::header::HeaderValue;
use log::warn;
use url::{Host, Url};

// TODO(bt, 2024-04-11): This is inefficient for middleware
pub fn netlify_branch_domain_matches(origin: &HeaderValue, netlify_hostname: &str) -> bool {
  let maybe_url = origin.to_str()
      .map(|origin| Url::parse(origin));

  let url = match maybe_url {
    Ok(Ok(url)) => url,
    _ => {
      warn!("Invalid origin: {:?}", origin);
      return false
    },
  };

  match url.host() {
    Some(Host::Domain(domain)) => {
      let is_netlify_domain = domain == netlify_hostname;
      if is_netlify_domain {
        return true;
      }

      let branch_suffix = format!("--{}", netlify_hostname);
      let is_netlify_branch_deploy = domain.ends_with(&branch_suffix);

      is_netlify_branch_deploy
    },
    _ => false,
  }
}
