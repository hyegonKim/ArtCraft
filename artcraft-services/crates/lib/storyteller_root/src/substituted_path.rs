use std::path::PathBuf;

use crate::{get_storyteller_frontend_root, get_storyteller_ml_root, get_storyteller_root, get_storyteller_rust_root};

const STORYTELLER_ROOT_REPLACEMENT : &str = "%storyteller_root%";

const STORYTELLER_FRONTEND_REPLACEMENT : &str = "%storyteller_frontend%";
const STORYTELLER_ML_REPLACEMENT : &str = "%storyteller_ml%";
const STORYTELLER_RUST_REPLACEMENT : &str = "%storyteller_rust%";

/// Transform a given string path, substituting "magic" variables for the various storyteller
/// project directories.
///
/// Substitution variables:
///
///  - %storyteller_root%
///  - %storyteller_rust%
///  - %storyteller_ml%
///  - %storyteller_frontend%
///
pub fn get_substituted_path(path: &str) -> PathBuf {
  // TODO(bt,2023-10-17): Should this be infallible? If not, where in the stack of transformations
  //  should the failure manifest? ie, if infallible, should we make the final paths "closer" to
  //  the de-substituted string, or the environment variable? Which makes it easier to detect the
  //  error?
  //
  // TODO(bt,2023-10-17): I'm not happy with all of the internal type juggling and allocations,
  //  but I shouldn't spend time on refactoring this now.
  //
  if path.contains(STORYTELLER_ROOT_REPLACEMENT) {
    let replacement = type_juggle_path(get_storyteller_root());
    let path = path.replacen(STORYTELLER_ROOT_REPLACEMENT, &replacement, 1);
    return PathBuf::from(path);
  } else if path.contains(STORYTELLER_RUST_REPLACEMENT) {
    let replacement = type_juggle_path(get_storyteller_rust_root());
    let path = path.replacen(STORYTELLER_RUST_REPLACEMENT, &replacement, 1);
    return PathBuf::from(path);
  } else if path.contains(STORYTELLER_ML_REPLACEMENT) {
    let replacement = type_juggle_path(get_storyteller_ml_root());
    let path = path.replacen(STORYTELLER_ML_REPLACEMENT, &replacement, 1);
    return PathBuf::from(path);
  } else if path.contains(STORYTELLER_FRONTEND_REPLACEMENT) {
    let replacement = type_juggle_path(get_storyteller_frontend_root());
    let path = path.replacen(STORYTELLER_FRONTEND_REPLACEMENT, &replacement, 1);
    return PathBuf::from(path);
  }

  PathBuf::from(path)
}

fn type_juggle_path(path: PathBuf) -> String {
  // NB: The Path/PathBuf library does some silly stuff regarding non-UTF-8 paths.
  path.to_str()
      .map(|s| s.to_string())
      .unwrap_or_else(|| path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use serial_test::serial;

  use crate::paths::storyteller_root::TEST_STORYTELLER_ROOT;
  use crate::substituted_path::get_substituted_path;

  #[test]
  fn test_no_substitutions() {
    assert_eq!(get_substituted_path("/foo/bar"), PathBuf::from("/foo/bar"));
  }

  #[test]
  #[serial]
  fn test_storyteller_root_substitution() {
    // NB: Careful with `set_var` as it escapes into *all* other tests! To make matters
    // more complicated, tests run in parallel and in any order. We're using a special
    // test variable so we never infect other tests.
    std::env::set_var(TEST_STORYTELLER_ROOT, "/testing/storyteller/root");

    assert_eq!(get_substituted_path("%storyteller_root%/foo/bar"), PathBuf::from("/testing/storyteller/root/foo/bar"));

    std::env::remove_var(TEST_STORYTELLER_ROOT);
  }

  #[test]
  #[serial]
  fn test_storyteller_rust_root_substitution() {
    // NB: Careful with `set_var` as it escapes into *all* other tests! To make matters
    // more complicated, tests run in parallel and in any order. We're using a special
    // test variable so we never infect other tests.
    std::env::set_var(TEST_STORYTELLER_ROOT, "/testing/storyteller/root");

    assert_eq!(get_substituted_path("%storyteller_rust%/foo/bar"), PathBuf::from("/testing/storyteller/root/storyteller-rust/foo/bar"));

    std::env::remove_var(TEST_STORYTELLER_ROOT);
  }

  #[test]
  #[serial]
  fn test_storyteller_ml_root_substitution() {
    // NB: Careful with `set_var` as it escapes into *all* other tests! To make matters
    // more complicated, tests run in parallel and in any order. We're using a special
    // test variable so we never infect other tests.
    std::env::set_var(TEST_STORYTELLER_ROOT, "/testing/storyteller/root");

    assert_eq!(get_substituted_path("%storyteller_ml%/foo/bar"), PathBuf::from("/testing/storyteller/root/storyteller-ml/foo/bar"));

    std::env::remove_var(TEST_STORYTELLER_ROOT);
  }

  #[test]
  #[serial]
  fn test_storyteller_frontend_root_substitution() {
    // NB: Careful with `set_var` as it escapes into *all* other tests! To make matters
    // more complicated, tests run in parallel and in any order. We're using a special
    // test variable so we never infect other tests.
    std::env::set_var(TEST_STORYTELLER_ROOT, "/testing/storyteller/root");

    assert_eq!(get_substituted_path("%storyteller_frontend%/foo/bar"), PathBuf::from("/testing/storyteller/root/storyteller-frontend/foo/bar"));

    std::env::remove_var(TEST_STORYTELLER_ROOT);
  }
}
