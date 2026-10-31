use std::path::{Path, PathBuf};

use crate::paths::env_get_path::env_get_path;

/// The env var that declares where the storyteller root directory lives.
pub const STORYTELLER_ROOT : &str = "STORYTELLER_ROOT";

/// Default OS home directory.
pub const HOME : &str = "HOME";

// DO NOT LEAK THIS. THIS IS FOR TESTING ONLY.
pub (crate) const TEST_STORYTELLER_ROOT : &str = "ENV_TEST_STORYTELLER_ROOT_DO_NOT_LEAK_";
pub (crate) const TEST_HOME : &str = "ENV_TEST_HOME_DO_NOT_LEAK_";

// TODO: Test on Mac, Windows, and WSL.
/// Use several heuristics to find "storyteller root", where all of our monorepos are located.
pub fn get_storyteller_root() -> PathBuf {
  // 1) Try "STORYTELLER_ROOT" env var override. Do not check for path existence.
  let env_var_name = if cfg!(test) { TEST_STORYTELLER_ROOT } else { STORYTELLER_ROOT };

  if let Ok(Some(path)) = env_get_path(env_var_name) {
    return path;
  }

  // TODO: Consider `dirs` or `directories` crates for cross-platform support:
  //  https://crates.io/crates/dirs
  //  https://crates.io/crates/directories

  // 2) Try to construct paths relative to the "HOME" directory. Prefer existing directories.
  let env_var_name = if cfg!(test) { TEST_HOME } else { HOME };

  if let Ok(Some(home_path)) = env_get_path(env_var_name) {
    if let Some(path) = try_subdirectories(home_path) {
      return path;
    }
  }

  // 3) Try to construct paths relative to the current working directory.
  if let Ok(current_working_dir) = std::env::current_dir() {
    if let Some(path) = try_subdirectories(current_working_dir) {
      return path;
    }
  }

  // 4) Rooted directory (since this function is infallible)
  PathBuf::from("/storyteller")
}

fn try_subdirectories(root_path: PathBuf) -> Option<PathBuf> {
  None
    // First try to see if the directories exist (in order)
    .or_else(|| try_subdirectory(&root_path, "code/storyteller", true))
    .or_else(|| try_subdirectory(&root_path, "dev/storyteller", true))
    // Then just use whatever parses.
    .or_else(|| try_subdirectory(&root_path, "code/storyteller", false))
}

fn try_subdirectory<'a>(dir: &Path, sub_path: &str, check_if_exists: bool) -> Option<PathBuf> {
  let mut home = dir.to_path_buf();
  home.push(sub_path);

  if let Ok(canonical) = home.canonicalize() {
    home = canonical; // NB: Ignore errors
  }

  if check_if_exists && !(home.exists() && home.is_dir()) {
    return None;
  }

  Some(home)
}

#[cfg(test)]
mod tests {
  use std::env::{current_dir, temp_dir};
  use std::fs::{create_dir, remove_dir};
  use std::path::PathBuf;

  use serial_test::serial;

  use crate::get_storyteller_root;
  use crate::paths::storyteller_root::{TEST_HOME, TEST_STORYTELLER_ROOT};

  #[test]
  #[serial]
  fn test_storyteller_home_env_var() {
    // NB: Careful with `set_var` as it escapes into *all* other tests! To make matters
    // more complicated, tests run in parallel and in any order. We're using a special
    // test variable so we never infect other tests.
    std::env::set_var(TEST_STORYTELLER_ROOT, "/testing/storyteller/root");
    assert_eq!(get_storyteller_root(), PathBuf::from("/testing/storyteller/root"));
    std::env::remove_var(TEST_STORYTELLER_ROOT);
  }

  #[test]
  #[serial]
  fn test_relative_to_home_directory_no_directories_exist() {
    // NB: Careful with `set_var` as it escapes into *all* other tests! To make matters
    // more complicated, tests run in parallel and in any order. We're using a special
    // test variable so we never infect other tests.
    std::env::set_var(TEST_HOME, "/testing/fake_home");
    assert_eq!(get_storyteller_root(), PathBuf::from("/testing/fake_home/code/storyteller"));
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

    let actual = get_storyteller_root();
    let mut expected = fake_storyteller.clone();

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

    let mut storyteller_dir = current_working_dir.clone();
    storyteller_dir.push("code/storyteller");

    assert_eq!(get_storyteller_root(), storyteller_dir);
  }
}
