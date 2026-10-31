use std::sync::Arc;

use crate::middleware::disabled_endpoint_filter::disabled_endpoints::exact_match_disabled_endpoints::ExactMatchDisabledEndpoints;
use crate::middleware::disabled_endpoint_filter::disabled_endpoints::prefix_disabled_endpoints::PrefixDisabledEndpoints;

#[derive(Clone)]
pub struct DisabledEndpoints {
  exact_match_endpoints: Arc<ExactMatchDisabledEndpoints>,
  prefix_endpoints: Arc<PrefixDisabledEndpoints>,
}

impl DisabledEndpoints {

  pub fn new(exact_match: ExactMatchDisabledEndpoints, prefix: PrefixDisabledEndpoints) -> Self {
    Self {
      exact_match_endpoints: Arc::new(exact_match),
      prefix_endpoints: Arc::new(prefix),
    }
  }

  pub fn endpoint_is_disabled(&self, endpoint: &str) -> bool {
    if self.exact_match_endpoints.endpoint_is_disabled(endpoint) {
      true
    } else if self.prefix_endpoints.endpoint_is_disabled(endpoint) {
      true
    } else {
      false
    }
  }
}
