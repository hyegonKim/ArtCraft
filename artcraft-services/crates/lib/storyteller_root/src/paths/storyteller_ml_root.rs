use std::path::PathBuf;

use crate::get_storyteller_root;
use crate::paths::env_get_path::env_get_path;

/// The env var that declares where the storyteller ml monorepo lives.
pub const STORYTELLER_ML_ROOT : &str = "STORYTELLER_ML_ROOT";

// DO NOT LEAK THIS. THIS IS FOR TESTING ONLY.
pub const TEST_STORYTELLER_ML_ROOT : &str = "ENV_TEST_STORYTELLER_ML_ROOT_DO_NOT_LEAK_";

/// Get the root of the Rust monorepo
pub fn get_storyteller_ml_root() -> PathBuf {
  // 1) Try "STORYTELLER_ML_ROOT" env var override. Do not check for path existence.
  let root = if cfg!(test) { TEST_STORYTELLER_ML_ROOT } else { STORYTELLER_ML_ROOT };

  if let Ok(Some(path)) = env_get_path(root) {
    return path;
  }

  // 2) Relative to "storyteller root"
  let mut dir = get_storyteller_root();
  dir.push("storyteller-ml");
  dir
}

#[cfg(test)]
mod tests {
  use std::env::{current_dir, temp_dir};
  use std::fs::{create_dir, remove_dir};
  use std::path::PathBuf;

  use serial_test::serial;

  use crate::paths::storyteller_ml_root::{get_storyteller_ml_root, TEST_STORYTELLER_ML_ROOT};
  use crate::paths::storyteller_root::TEST_HOME;

  #[test]
  #[serial]
  fn test_storyteller_ml_home_env_var() {
    // NB: Careful with `set_var` as it escapes into *all* other tests! To make matters
    // more complicated, tests run in parallel and in any order. We're using a special
    // test variable so we never infect other tests.
    std::env::set_var(TEST_STORYTELLER_ML_ROOT, "/testing/storyteller/root/custom-storyteller-ml");
    assert_eq!(get_storyteller_ml_root(), PathBuf::from("/testing/storyteller/root/custom-storyteller-ml"));
    std::env::remove_var(TEST_STORYTELLER_ML_ROOT);
  }

  #[test]
  #[serial]
  fn test_relative_to_home_directory_no_directories_exist() {
    // NB: Careful with `set_var` as it escapes into *all* other tests! To make matters
    // more complicated, tests run in parallel and in any order. We're using a special
    // test variable so we never infect other tests.
    std::env::set_var(TEST_HOME, "/testing/fake_home");
    assert_eq!(get_storyteller_ml_root(), PathBuf::from("/testing/fake_home/code/storyteller/storyteller-ml"));
    std::env::remove_var(TEST_HOME);
  }

  #[test]
  #[serial]
  fn test_relative_to_home_directory_with_existing_directory() {
    let mut fake_home = temp_dir();
    fake_home.push("temp_dir_for_testing");

    let mut fake_dev = fake_home.clone();
    fake_dev.push("dev");

    // NB: Final path should be /tmp/temp_dir_for_testing/dev/storyteller
    let mut fake_storyteller = fake_dev.clone();
    fake_storyteller.push("storyteller");

    // NB: Ordered for repeatability
    let _r = remove_dir(&fake_home).ok();
    let _r = create_dir(&fake_home).ok();
    let _r = create_dir(&fake_dev).ok();
    let _r = create_dir(&fake_storyteller).ok();

    // NB: Careful with `set_var` as it escapes into *all* other tests! To make matters
    // more complicated, tests run in parallel and in any order. We're using a special
    // test variable so we never infect other tests.
    std::env::set_var(TEST_HOME, fake_home.to_str().expect("should be string"));

    let mut expected = fake_storyteller.clone();
    expected.push("storyteller-ml");

    let actual = get_storyteller_ml_root();

    if actual.starts_with("/private") && !expected.starts_with("/private") {
      // NB: For Mac, TempDir creates in /var/... which is an alias of /private/var/...
      // We'll make sure to canonicalize both paths.
      let mut corrected = PathBuf::from("/private");

      expected.iter()
          .filter(|component| !component.to_string_lossy().eq("/"))
          .for_each(|component| corrected.push(component));

      expected = corrected;
    }

    assert_eq!(actual, expected);

    std::env::remove_var(TEST_HOME);

    // NB: Must remove in reverse order, or we leave dangling pointers
    let _r = remove_dir(&fake_storyteller).ok();
    let _r = remove_dir(&fake_dev).ok();
    let _r = remove_dir(&fake_home).ok();
  }

  #[test]
  #[serial]
  fn test_relative_to_current_working_directory() {
    // NB: In this test we have no "TEST_HOME" directory.
    let current_working_dir = current_dir().expect("Should have working dir.");

    let mut expected_dir = current_working_dir.clone();
    expected_dir.push("code/storyteller/storyteller-ml");

    assert_eq!(get_storyteller_ml_root(), expected_dir);
  }
}
