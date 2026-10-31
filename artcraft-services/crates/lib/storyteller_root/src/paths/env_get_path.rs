use std::env::VarError;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;

/// Errors with reading and parsing env variables.
#[derive(Debug, Eq, PartialEq)]
pub enum EnvError {
  /// The environment variable value is not unicode.
  NotUnicode,
  /// Problem parsing the env variable as the desired type.
  ParseError {
    /// Explanation of the parsing failure.
    reason: String
  },
  /// The required environment variable wasn't present.
  RequiredNotPresent,
}

impl Display for EnvError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let reason = match self {
      EnvError::NotUnicode => "EnvError::NotUnicode",
      EnvError::ParseError { .. } => "EnvError::ParseError",
      EnvError::RequiredNotPresent => "EnvError::RequiredNotPresent",
    };
    write!(f, "{:?}", reason)
  }
}

impl Error for EnvError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    None
  }
}

// NB: Pulled from `easyenv` crate.
pub (crate) fn env_get_path(env_name: &str) -> Result<Option<PathBuf>, EnvError> {
  match std::env::var(env_name).as_ref() {
    Err(err) => match err {
      // TODO: EnvError enum variant for equals sign in env var name
      VarError::NotPresent => Ok(None),
      VarError::NotUnicode(_) => Err(EnvError::NotUnicode),
    }
    Ok(val) => {
      match PathBuf::from_str(val) {
        Ok(path) => Ok(Some(path)),
        Err(_err) => Err(EnvError::ParseError {
          reason: "error parsing PathBuf from value".to_string()
        }),
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use super::env_get_path;

  #[test]
  fn test_read_unset_env() {
    assert_eq!(env_get_path("ASDFGH_FOO"), Ok(None));
  }

  #[test]
  fn test_read_set_env() {
    // NB: Careful with `set_var` as it escapes into *all* other tests! To make matters
    // more complicated, tests run in parallel and in any order. We're using a special
    // test variable so we never infect other tests.
    const ENV_VAR_NAME : &str = "ENV_TEST_VAR_DO_NOT_REUSE";

    std::env::set_var(ENV_VAR_NAME, "/foo/bar");

    assert_eq!(env_get_path(ENV_VAR_NAME), Ok(Some(PathBuf::from("/foo/bar"))));

    std::env::remove_var(ENV_VAR_NAME);
  }
}
