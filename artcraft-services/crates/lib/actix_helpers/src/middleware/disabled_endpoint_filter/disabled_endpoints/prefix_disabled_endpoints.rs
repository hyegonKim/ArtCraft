use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use errors::AnyhowResult;

pub struct PrefixDisabledEndpoints {
  endpoint_prefixes: HashSet<String>
}

impl PrefixDisabledEndpoints {
  pub fn new() -> Self {
    Self {
      endpoint_prefixes: HashSet::new()
    }
  }

  pub fn from_set(endpoint_prefixes: HashSet<String>) -> Self {
    Self {
      endpoint_prefixes,
    }
  }

  pub fn load_from_file<P: AsRef<Path>>(path: P) -> AnyhowResult<Self> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let lines = reader.lines()
        .filter_map(|line| line.ok())
        .map(|line| line.trim().to_string())
        .filter(|line| !(line.starts_with("#") || line.is_empty()))
        .collect::<HashSet<String>>();

    Ok(Self::from_set(lines))
  }

  pub fn add_endpoint(&mut self, endpoint: String) -> bool {
    self.endpoint_prefixes.insert(endpoint)
  }

  pub fn endpoint_is_disabled(&self, endpoint: &str) -> bool {
    self.endpoint_prefixes.iter()
        .any(|prefix| endpoint.starts_with(prefix))
  }

  pub fn len(&self) -> usize {
    self.endpoint_prefixes.len()
  }
}

#[cfg(test)]
pub mod tests {
  use crate::middleware::disabled_endpoint_filter::disabled_endpoints::prefix_disabled_endpoints::PrefixDisabledEndpoints;

  #[test]
  fn test_endpoint_is_disabled() {
    let mut endpoints = PrefixDisabledEndpoints::new();
    endpoints.add_endpoint("/foo".to_string());
    endpoints.add_endpoint("/this/is/a/test".to_string());

    // Disabled
    assert_eq!(endpoints.endpoint_is_disabled("/foo"), true);
    assert_eq!(endpoints.endpoint_is_disabled("/this/is/a/test"), true);

    // Also disabled due to "starts with"
    assert_eq!(endpoints.endpoint_is_disabled("/foo/"), true);
    assert_eq!(endpoints.endpoint_is_disabled("/foo/bar"), true);
    assert_eq!(endpoints.endpoint_is_disabled("/this/is/a/test/again"), true);

    // Not disabled
    assert_eq!(endpoints.endpoint_is_disabled("/"), false);
    assert_eq!(endpoints.endpoint_is_disabled("/bar"), false);
    assert_eq!(endpoints.endpoint_is_disabled("/this/is/not/a/test"), false);

    // Metadata
    assert_eq!(endpoints.len(), 2);
  }
}
