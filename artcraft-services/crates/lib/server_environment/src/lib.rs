//! server_environment
//!
//! A very core enum that specifies where a service is running.
//!

// Never allow these
#![forbid(private_bounds)]
#![forbid(private_interfaces)]
#![forbid(unused_must_use)] // NB: It's unsafe to not close/check some things

// Okay to toggle
#![forbid(unreachable_patterns)]
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

// Always allow
#![allow(dead_code)]
#![allow(non_snake_case)]


/// The environmental context the server is operating in.
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum ServerEnvironment {
  /// Development instances of our web services run on developer machines.
  Development,

  /// The production machines behind our web services.
  Production,

  // /// NB: Staging does not yet exist, and we sort of treat production as a semi-staging
  // /// for frontend testing.
  // Staging,
}

impl ServerEnvironment {
  pub fn from_str(environment: &str) -> Option<ServerEnvironment> {
    match environment {
      "dev" | "DEV" | "development" | "DEVELOPMENT" => Some(ServerEnvironment::Development),
      "prod" | "PROD" | "production" | "PRODUCTION" => Some(ServerEnvironment::Production),
      // "stage" | "STAGE" | "staging" | "STAGING" => Some(ServerEnvironment::Staging),
      _ => None,
    }
  }

  /// Reports whether the software is running in production.
  /// This is useful once proper "staging" backends gets introduced, as currently staging frontend
  /// is served by the production backend.
  pub fn is_deployed_in_production(&self) -> bool {
    match self {
      ServerEnvironment::Development => false,
      ServerEnvironment::Production => true,
    }
  }

  /// Reports whether the software is running in development.
  pub fn is_development(&self) -> bool {
    match self {
      ServerEnvironment::Development => true,
      ServerEnvironment::Production => false,
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::ServerEnvironment;

  #[test]
  fn test_development_from_str() {
    assert_eq!(ServerEnvironment::from_str("dev"), Some(ServerEnvironment::Development));
    assert_eq!(ServerEnvironment::from_str("development"), Some(ServerEnvironment::Development));
    assert_eq!(ServerEnvironment::from_str("DEV"), Some(ServerEnvironment::Development));
    assert_eq!(ServerEnvironment::from_str("DEVELOPMENT"), Some(ServerEnvironment::Development));
  }

  #[test]
  fn test_production_from_str() {
    assert_eq!(ServerEnvironment::from_str("prod"), Some(ServerEnvironment::Production));
    assert_eq!(ServerEnvironment::from_str("production"), Some(ServerEnvironment::Production));
    assert_eq!(ServerEnvironment::from_str("PROD"), Some(ServerEnvironment::Production));
    assert_eq!(ServerEnvironment::from_str("PRODUCTION"), Some(ServerEnvironment::Production));
  }

  #[test]
  fn test_invalid() {
    assert_eq!(ServerEnvironment::from_str(""), None);
    assert_eq!(ServerEnvironment::from_str("develops"), None);
    assert_eq!(ServerEnvironment::from_str("pRoDs"), None);
    assert_eq!(ServerEnvironment::from_str("staging"), None);
  }

  #[test]
  fn test_is_deployed_in_production() {
    assert_eq!(true, ServerEnvironment::Production.is_deployed_in_production());
    assert_eq!(false, ServerEnvironment::Development.is_deployed_in_production());
  }

  #[test]
  fn test_is_development() {
    assert_eq!(false, ServerEnvironment::Production.is_development());
    assert_eq!(true, ServerEnvironment::Development.is_development());
  }
}
