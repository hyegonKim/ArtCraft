use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use errors::AnyhowResult;

pub struct ExactMatchDisabledEndpoints {
  endpoints: HashSet<String>
}

impl ExactMatchDisabledEndpoints {
  pub fn new() -> Self {
    Self {
      endpoints: HashSet::new()
    }
  }

  pub fn from_set(endpoints: HashSet<String>) -> Self {
    Self {
      endpoints,
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
    self.endpoints.insert(endpoint)
  }

  pub fn endpoint_is_disabled(&self, endpoint: &str) -> bool {
    self.endpoints.contains(endpoint)
  }

  pub fn len(&self) -> usize {
    self.endpoints.len()
  }
}

#[cfg(test)]
pub mod tests {
  use crate::middleware::disabled_endpoint_filter::disabled_endpoints::exact_match_disabled_endpoints::ExactMatchDisabledEndpoints;

  #[test]
  fn test_endpoint_is_disabled() {
    let mut endpoints = ExactMatchDisabledEndpoints::new();
    endpoints.add_endpoint("/foo".to_string());
    endpoints.add_endpoint("/this/is/a/test".to_string());

    // Disabled
    assert_eq!(endpoints.endpoint_is_disabled("/foo"), true);
    assert_eq!(endpoints.endpoint_is_disabled("/this/is/a/test"), true);

    // Not disabled
    assert_eq!(endpoints.endpoint_is_disabled("/bar"), false);
    assert_eq!(endpoints.endpoint_is_disabled("/foo/"), false);
    assert_eq!(endpoints.endpoint_is_disabled("/foo/bar"), false);
    assert_eq!(endpoints.endpoint_is_disabled("/this/is/a/test/again"), false);

    // Stats
    assert_eq!(endpoints.len(), 2);
  }
}
