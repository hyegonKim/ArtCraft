//! Static FakeYou API tokens can be read from a config file.
//! This will be a stopgap until we deploy the full system.

use std::collections::HashMap;

use log::{error, info};

use crate::util::read_toml_file_to_struct::read_toml_file_to_struct;

/// Config to pass to handlers
pub struct StaticApiTokenSet {
  /// Token -> Token config
  api_tokens: HashMap<String, StaticApiTokenConfig>
}

impl StaticApiTokenSet {
  /// Read from file
  pub fn from_file(filename: &str) -> Self {
    let api_tokens = read_toml_file_to_struct(filename).unwrap_or_else(|e| {
      error!("Error reading static API tokens config file: {:?}", e);
      StaticApiTokens::default()
    });

    info!("Static API Tokens: {} total", api_tokens.api_tokens.len());

    let mut map = HashMap::new();
    for item in api_tokens.api_tokens.into_iter() {
      info!("{:?}", &item);
      map.insert(item.api_token.clone(), item);
    }

    Self {
      api_tokens: map,
    }
  }
  pub fn get_api_token(&self, api_token: &str) -> Option<StaticApiTokenConfig> {
    self.api_tokens.get(api_token).map(|token| token.clone())
  }
}

/// Struct deserialization of TOML config file
#[derive(Clone, Deserialize, Debug, Default)]
pub struct StaticApiTokens {
  pub api_tokens: Vec<StaticApiTokenConfig>
}

#[derive(Clone, Deserialize, Debug)]
pub struct StaticApiTokenConfig {
  /// API token.
  pub api_token: String,

  /// Force this user token if present.
  pub maybe_user_token: Option<String>,

  /// Priority level to force
  /// Defaults to "1" if not set.
  pub maybe_priority_level: Option<u8>,

  /// Use a higher priority rate limiter (with higher QPS)
  /// Defaults to "false" if not set.
  pub maybe_use_high_priority_rate_limiter: Option<bool>,

  /// Disable rate limiter?
  /// Defaults to "false" if not set.
  pub maybe_disable_rate_limiter: Option<bool>,
}
