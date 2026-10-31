use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};

/// Errors with reading and parsing env variables.
#[derive(Debug)]
pub enum EnvError {
  /// The environment variable value is not unicode.
  NotUnicode,
  /// Problem parsing the env variable as the desired type.
  ParseError {
    /// Explanation of the parsing failure.
    reason: String
  },
  /// The required environment variable wasn't present.
  RequiredNotPresent {
    /// The name of the missing environment variable.
    name: String
  },
}

impl Display for EnvError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let reason = match self {
      EnvError::NotUnicode => "EnvError::NotUnicode",
      EnvError::ParseError { .. } => "EnvError::ParseError",
      EnvError::RequiredNotPresent { name } =>
        return write!(f, r#"
          EnvError::RequiredNotPresent: the following environment variable was not present:

              --->  {:?}

          In development, setting it in the environment config files: .env, .env-secrets,
              cargo/service/../{{app_name}}/config/{{app_name}}.development.env, etc.

          If it looks like it might be a secret, ask for help in our company Discord.

          In production, make sure these environment variables are set in Kubernetes.
        "#, name),
    };
    write!(f, "{:?}", reason)
  }
}

impl Error for EnvError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    None
  }
}

/// Errors while initializing the library
#[derive(Debug)]
pub enum InitError {
  /// std::io error
  IoError,
  /// dotenv couldn't read a file
  DotEnvError,
  /// No env config file was found and read
  NoConfigFileFoundError,
}

impl Display for InitError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let reason = match self {
      InitError::IoError => "InitError::IoError",
      InitError::DotEnvError => "InitError::DotEnvError",
      InitError::NoConfigFileFoundError => "InitError::NoConfigFileFoundError",
    };
    write!(f, "{:?}", reason)
  }
}

impl Error for InitError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    None
  }
}
